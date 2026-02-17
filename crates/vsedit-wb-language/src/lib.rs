//! Language registration service.

use std::fmt;

/// Errors that can occur during language registration and lookup.
#[derive(Debug, Clone, PartialEq)]
pub enum LanguageError {
    /// The language id is empty.
    EmptyId,
    /// The language name is empty.
    EmptyName,
    /// A language with this id is already registered.
    DuplicateId(String),
    /// No language found for the given query.
    NotFound(String),
}

impl fmt::Display for LanguageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LanguageError::EmptyId => write!(f, "language id must not be empty"),
            LanguageError::EmptyName => write!(f, "language name must not be empty"),
            LanguageError::DuplicateId(id) => write!(f, "language '{}' is already registered", id),
            LanguageError::NotFound(q) => write!(f, "no language found for '{}'", q),
        }
    }
}

impl std::error::Error for LanguageError {}

/// Metadata for a single language.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageInfo {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
    pub aliases: Vec<String>,
    pub mime_types: Vec<String>,
    pub first_line_pattern: Option<String>,
}

impl LanguageInfo {
    /// Returns `true` if any registered extension matches the filename's extension.
    pub fn matches_filename(&self, filename: &str) -> bool {
        self.extensions
            .iter()
            .any(|ext| filename.ends_with(ext.as_str()))
    }

    /// Returns `true` if `alias` is among this language's aliases.
    pub fn has_alias(&self, alias: &str) -> bool {
        self.aliases.iter().any(|a| a == alias)
    }

    /// Validates that id and name are non-empty.
    pub fn validate(&self) -> Result<(), LanguageError> {
        if self.id.trim().is_empty() {
            return Err(LanguageError::EmptyId);
        }
        if self.name.trim().is_empty() {
            return Err(LanguageError::EmptyName);
        }
        Ok(())
    }

    /// Returns the primary file extension, if any.
    pub fn primary_extension(&self) -> Option<&str> {
        self.extensions.first().map(|s| s.as_str())
    }

    /// Returns the primary MIME type, if any.
    pub fn primary_mime_type(&self) -> Option<&str> {
        self.mime_types.first().map(|s| s.as_str())
    }

    /// Returns the total count of extensions, aliases, and mime types.
    pub fn metadata_count(&self) -> usize {
        self.extensions.len() + self.aliases.len() + self.mime_types.len()
    }

    /// Returns `true` if this language has any registered MIME type.
    pub fn has_mime_types(&self) -> bool {
        !self.mime_types.is_empty()
    }
}

impl std::fmt::Display for LanguageInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.id)
    }
}

/// Builder for constructing a [`LanguageInfo`] step by step.
#[derive(Debug, Clone, Default)]
pub struct LanguageInfoBuilder {
    id: String,
    name: String,
    extensions: Vec<String>,
    aliases: Vec<String>,
    mime_types: Vec<String>,
    first_line_pattern: Option<String>,
}

impl LanguageInfoBuilder {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn extension(mut self, ext: impl Into<String>) -> Self {
        self.extensions.push(ext.into());
        self
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    pub fn mime_type(mut self, mime: impl Into<String>) -> Self {
        self.mime_types.push(mime.into());
        self
    }

    pub fn first_line_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.first_line_pattern = Some(pattern.into());
        self
    }

    /// Builds and validates the [`LanguageInfo`].
    pub fn build(self) -> Result<LanguageInfo, LanguageError> {
        let info = LanguageInfo {
            id: self.id,
            name: self.name,
            extensions: self.extensions,
            aliases: self.aliases,
            mime_types: self.mime_types,
            first_line_pattern: self.first_line_pattern,
        };
        info.validate()?;
        Ok(info)
    }
}

impl fmt::Display for LanguageInfoBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LanguageInfoBuilder({}, {})", self.id, self.name)
    }
}

/// Registry that holds all known languages.
#[derive(Debug, Clone, Default)]
pub struct LanguageRegistry {
    pub languages: Vec<LanguageInfo>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self {
            languages: Vec::new(),
        }
    }

    pub fn register(&mut self, info: LanguageInfo) {
        self.languages.push(info);
    }

    pub fn get_language(&self, id: &str) -> Option<&LanguageInfo> {
        self.languages.iter().find(|l| l.id == id)
    }

    pub fn get_by_extension(&self, ext: &str) -> Option<&LanguageInfo> {
        self.languages
            .iter()
            .find(|l| l.extensions.iter().any(|e| e == ext))
    }

    pub fn get_by_mime_type(&self, mime: &str) -> Option<&LanguageInfo> {
        self.languages
            .iter()
            .find(|l| l.mime_types.iter().any(|m| m == mime))
    }

    pub fn get_all_ids(&self) -> Vec<&str> {
        self.languages.iter().map(|l| l.id.as_str()).collect()
    }

    pub fn language_count(&self) -> usize {
        self.languages.len()
    }

    pub fn get_by_alias(&self, alias: &str) -> Option<&LanguageInfo> {
        self.languages.iter().find(|l| l.has_alias(alias))
    }

    pub fn get_by_filename(&self, filename: &str) -> Option<&LanguageInfo> {
        self.languages.iter().find(|l| l.matches_filename(filename))
    }

    /// Checks if the first line of a file contains the language's `first_line_pattern`.
    pub fn get_by_first_line(&self, first_line: &str) -> Option<&LanguageInfo> {
        self.languages.iter().find(|l| {
            l.first_line_pattern
                .as_ref()
                .map_or(false, |pat| first_line.contains(pat.as_str()))
        })
    }

    /// Removes a language by id. Returns `true` if a language was removed.
    pub fn unregister(&mut self, id: &str) -> bool {
        let before = self.languages.len();
        self.languages.retain(|l| l.id != id);
        self.languages.len() < before
    }

    /// Returns every extension across all registered languages.
    pub fn get_all_extensions(&self) -> Vec<&str> {
        self.languages
            .iter()
            .flat_map(|l| l.extensions.iter().map(|e| e.as_str()))
            .collect()
    }

    pub fn has_language(&self, id: &str) -> bool {
        self.languages.iter().any(|l| l.id == id)
    }

    /// Case-insensitive search across id, name, and aliases.
    pub fn search(&self, query: &str) -> Vec<&LanguageInfo> {
        let q = query.to_lowercase();
        self.languages
            .iter()
            .filter(|l| {
                l.id.to_lowercase().contains(&q)
                    || l.name.to_lowercase().contains(&q)
                    || l.aliases.iter().any(|a| a.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Registers a language after validating it and checking for duplicate ids.
    pub fn try_register(&mut self, info: LanguageInfo) -> Result<(), LanguageError> {
        info.validate()?;
        if self.has_language(&info.id) {
            return Err(LanguageError::DuplicateId(info.id.clone()));
        }
        self.languages.push(info);
        Ok(())
    }

    /// Returns a language by id, or an error if not found.
    pub fn require_language(&self, id: &str) -> Result<&LanguageInfo, LanguageError> {
        self.get_language(id)
            .ok_or_else(|| LanguageError::NotFound(id.to_string()))
    }

    /// Returns all unique MIME types across registered languages.
    pub fn get_all_mime_types(&self) -> Vec<&str> {
        let mut mimes: Vec<&str> = self
            .languages
            .iter()
            .flat_map(|l| l.mime_types.iter().map(|m| m.as_str()))
            .collect();
        mimes.sort();
        mimes.dedup();
        mimes
    }

    /// Returns languages sorted alphabetically by name.
    pub fn sorted_by_name(&self) -> Vec<&LanguageInfo> {
        let mut sorted: Vec<&LanguageInfo> = self.languages.iter().collect();
        sorted.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        sorted
    }

    /// Merges all languages from `other` into this registry, skipping duplicates.
    pub fn merge(&mut self, other: &LanguageRegistry) {
        for lang in &other.languages {
            if !self.has_language(&lang.id) {
                self.languages.push(lang.clone());
            }
        }
    }
}

/// Summary statistics for the status of languages in a registry.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageStatusStats {
    /// Total number of registered language items.
    pub total_items: usize,
    /// Number of languages that have at least one extension registered.
    pub active_count: usize,
    /// Number of languages that failed validation (empty id or name).
    pub error_count: usize,
}

impl fmt::Display for LanguageStatusStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "languages: {} total, {} active, {} errors",
            self.total_items, self.active_count, self.error_count
        )
    }
}

/// Computes status statistics for a slice of [`LanguageInfo`] items.
///
/// A language is considered *active* if it has at least one file extension.
/// A language is counted as an *error* if its validation fails.
pub fn compute_language_status_stats(languages: &[LanguageInfo]) -> LanguageStatusStats {
    let total_items = languages.len();
    let mut active_count = 0;
    let mut error_count = 0;
    for lang in languages {
        if lang.validate().is_err() {
            error_count += 1;
        } else if !lang.extensions.is_empty() {
            active_count += 1;
        }
    }
    LanguageStatusStats {
        total_items,
        active_count,
        error_count,
    }
}

/// Statistics about languages in a registry.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageStatistics {
    /// Total number of registered languages.
    pub total: usize,
    /// Total number of extensions across all languages.
    pub total_extensions: usize,
    /// Total number of aliases across all languages.
    pub total_aliases: usize,
    /// Total number of MIME types across all languages.
    pub total_mime_types: usize,
    /// Number of languages with a first-line pattern.
    pub with_first_line_pattern: usize,
}

/// Computes detailed statistics for a language registry.
pub fn compute_language_statistics(registry: &LanguageRegistry) -> LanguageStatistics {
    let mut total_extensions = 0;
    let mut total_aliases = 0;
    let mut total_mime_types = 0;
    let mut with_first_line_pattern = 0;
    for lang in &registry.languages {
        total_extensions += lang.extensions.len();
        total_aliases += lang.aliases.len();
        total_mime_types += lang.mime_types.len();
        if lang.first_line_pattern.is_some() {
            with_first_line_pattern += 1;
        }
    }
    LanguageStatistics {
        total: registry.language_count(),
        total_extensions,
        total_aliases,
        total_mime_types,
        with_first_line_pattern,
    }
}

/// Represents a category grouping for languages.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageGroup {
    pub category: String,
    pub language_ids: Vec<String>,
}

/// Groups languages by the first character of their name (uppercased).
pub fn group_languages_by_initial(registry: &LanguageRegistry) -> Vec<LanguageGroup> {
    let mut map: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for lang in &registry.languages {
        let initial = lang.name.chars().next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".to_string());
        map.entry(initial).or_default().push(lang.id.clone());
    }
    map.into_iter()
        .map(|(category, language_ids)| LanguageGroup { category, language_ids })
        .collect()
}

/// Resolves a language alias to its canonical language id.
pub fn resolve_alias(registry: &LanguageRegistry, alias: &str) -> Option<String> {
    registry.get_by_alias(alias).map(|l| l.id.clone())
}

/// Feature flags that a language may support.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageFeatureMatrix {
    pub language_id: String,
    pub has_folding: bool,
    pub has_indentation_rules: bool,
    pub has_bracket_pairs: bool,
    pub has_auto_closing_pairs: bool,
}

impl LanguageFeatureMatrix {
    /// Creates a new feature matrix with all features disabled.
    pub fn new(language_id: impl Into<String>) -> Self {
        Self {
            language_id: language_id.into(),
            has_folding: false,
            has_indentation_rules: false,
            has_bracket_pairs: false,
            has_auto_closing_pairs: false,
        }
    }

    /// Returns the number of enabled features.
    pub fn enabled_count(&self) -> usize {
        [self.has_folding, self.has_indentation_rules, self.has_bracket_pairs, self.has_auto_closing_pairs]
            .iter()
            .filter(|&&v| v)
            .count()
    }
}

// ---------------------------------------------------------------------------
// Language association
// ---------------------------------------------------------------------------

/// Maps a file extension to a language ID for quick lookup.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageAssociation {
    /// File extension including the dot, e.g. ".rs".
    pub extension: String,
    /// Language ID this extension maps to, e.g. "rust".
    pub language_id: String,
}

impl LanguageAssociation {
    pub fn new(extension: impl Into<String>, language_id: impl Into<String>) -> Self {
        Self {
            extension: extension.into(),
            language_id: language_id.into(),
        }
    }
}

/// Resolve a file extension to a language ID using a list of associations.
///
/// Returns the language ID of the first matching association, or `None`.
pub fn resolve_language_by_extension(
    associations: &[LanguageAssociation],
    filename: &str,
) -> Option<String> {
    let lower = filename.to_lowercase();
    associations
        .iter()
        .find(|a| lower.ends_with(&a.extension.to_lowercase()))
        .map(|a| a.language_id.clone())
}

/// A first-line pattern rule mapping a prefix to a language ID.
#[derive(Debug, Clone)]
struct FirstLineRule {
    pattern: String,
    language_id: String,
}

/// Guesses a language from the first line of a file using pattern matching.
#[derive(Debug, Clone)]
pub struct LanguageDetector {
    rules: Vec<FirstLineRule>,
}

impl LanguageDetector {
    /// Creates a detector with built-in rules for common shebangs and file headers.
    pub fn new() -> Self {
        let rules = vec![
            FirstLineRule {
                pattern: "#!/usr/bin/env python".into(),
                language_id: "python".into(),
            },
            FirstLineRule {
                pattern: "#!/bin/bash".into(),
                language_id: "shellscript".into(),
            },
            FirstLineRule {
                pattern: "#!/usr/bin/env node".into(),
                language_id: "javascript".into(),
            },
            FirstLineRule {
                pattern: "<?xml".into(),
                language_id: "xml".into(),
            },
            FirstLineRule {
                pattern: "<!DOCTYPE html".into(),
                language_id: "html".into(),
            },
            FirstLineRule {
                pattern: "{".into(),
                language_id: "json".into(),
            },
        ];
        Self { rules }
    }

    /// Detects a language from a single first line.
    pub fn detect_from_first_line(&self, line: &str) -> Option<String> {
        let trimmed = line.trim();
        for rule in &self.rules {
            if trimmed.starts_with(&rule.pattern) {
                return Some(rule.language_id.clone());
            }
        }
        None
    }

    /// Detects a language from file content by inspecting the first line.
    pub fn detect_from_content(&self, content: &str) -> Option<String> {
        let first_line = content.lines().next().unwrap_or("");
        self.detect_from_first_line(first_line)
    }
}

impl Default for LanguageDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Given a slice of associations, find extensions that map to multiple language IDs.
///
/// Returns a list of `(extension, [language_ids])` tuples sorted by extension,
/// including only extensions with two or more associated languages.
pub fn language_overlap(associations: &[LanguageAssociation]) -> Vec<(String, Vec<String>)> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for assoc in associations {
        let ext = assoc.extension.to_lowercase();
        let entry = map.entry(ext).or_default();
        if !entry.contains(&assoc.language_id) {
            entry.push(assoc.language_id.clone());
        }
    }
    map.into_iter().filter(|(_, ids)| ids.len() > 1).collect()
}

/// Tracks which editor features a language supports.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LanguageFeatureSupport {
    pub completion: bool,
    pub hover: bool,
    pub formatting: bool,
    pub diagnostics: bool,
    pub go_to_definition: bool,
    pub references: bool,
    pub rename: bool,
    pub code_actions: bool,
}

impl LanguageFeatureSupport {
    /// Returns how many features are enabled.
    pub fn supports_count(&self) -> usize {
        [
            self.completion,
            self.hover,
            self.formatting,
            self.diagnostics,
            self.go_to_definition,
            self.references,
            self.rename,
            self.code_actions,
        ]
        .iter()
        .filter(|&&v| v)
        .count()
    }
}

// ---------------------------------------------------------------------------
// LanguageInfo extensions
// ---------------------------------------------------------------------------

impl LanguageInfo {
    pub fn has_aliases(&self) -> bool {
        !self.aliases.is_empty()
    }

    pub fn alias_count(&self) -> usize {
        self.aliases.len()
    }

    pub fn extension_count(&self) -> usize {
        self.extensions.len()
    }

    /// Case-insensitive search across id, name, aliases, and extensions.
    pub fn matches_filter(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.id.to_lowercase().contains(&q)
            || self.name.to_lowercase().contains(&q)
            || self.aliases.iter().any(|a| a.to_lowercase().contains(&q))
            || self.extensions.iter().any(|e| e.to_lowercase().contains(&q))
    }
}

// ---------------------------------------------------------------------------
// LanguageRegistry iterator support
// ---------------------------------------------------------------------------

impl LanguageRegistry {
    pub fn iter(&self) -> std::slice::Iter<'_, LanguageInfo> {
        self.languages.iter()
    }

    pub fn all_extensions(&self) -> Vec<String> {
        let mut exts: Vec<String> = self
            .languages
            .iter()
            .flat_map(|l| l.extensions.clone())
            .collect();
        exts.sort();
        exts.dedup();
        exts
    }

    pub fn find_by_alias(&self, alias: &str) -> Vec<&LanguageInfo> {
        let lower = alias.to_lowercase();
        self.languages
            .iter()
            .filter(|l| l.aliases.iter().any(|a| a.to_lowercase() == lower))
            .collect()
    }
}

impl<'a> IntoIterator for &'a LanguageRegistry {
    type Item = &'a LanguageInfo;
    type IntoIter = std::slice::Iter<'a, LanguageInfo>;

    fn into_iter(self) -> Self::IntoIter {
        self.languages.iter()
    }
}

// ---------------------------------------------------------------------------
// LanguageGroup extensions
// ---------------------------------------------------------------------------

impl LanguageGroup {
    pub fn total_languages(&self) -> usize {
        self.language_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.language_ids.is_empty()
    }
}

impl fmt::Display for LanguageGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} languages)", self.category, self.language_ids.len())
    }
}

// ---------------------------------------------------------------------------
// LanguageFeatureMatrix extensions
// ---------------------------------------------------------------------------

impl LanguageFeatureMatrix {
    pub fn supported_features(&self) -> Vec<&'static str> {
        let mut features = Vec::new();
        if self.has_folding { features.push("folding"); }
        if self.has_indentation_rules { features.push("indentation_rules"); }
        if self.has_bracket_pairs { features.push("bracket_pairs"); }
        if self.has_auto_closing_pairs { features.push("auto_closing_pairs"); }
        features
    }

    pub fn feature_count(&self) -> usize {
        4
    }

    pub fn is_fully_supported(&self) -> bool {
        self.has_folding
            && self.has_indentation_rules
            && self.has_bracket_pairs
            && self.has_auto_closing_pairs
    }
}

// ---------------------------------------------------------------------------
// LanguageAssociation extensions
// ---------------------------------------------------------------------------

impl fmt::Display for LanguageAssociation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.extension, self.language_id)
    }
}

impl LanguageAssociation {
    pub fn is_extension_based(&self) -> bool {
        self.extension.starts_with('.')
    }
}

// ---------------------------------------------------------------------------
// LanguageDetector extensions
// ---------------------------------------------------------------------------

impl LanguageDetector {
    pub fn add_rule(&mut self, pattern: impl Into<String>, language_id: impl Into<String>) {
        self.rules.push(FirstLineRule {
            pattern: pattern.into(),
            language_id: language_id.into(),
        });
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

// ---------------------------------------------------------------------------
// LanguageStatistics extensions
// ---------------------------------------------------------------------------

impl LanguageStatistics {
    pub fn merge(&self, other: &LanguageStatistics) -> LanguageStatistics {
        LanguageStatistics {
            total: self.total + other.total,
            total_extensions: self.total_extensions + other.total_extensions,
            total_aliases: self.total_aliases + other.total_aliases,
            total_mime_types: self.total_mime_types + other.total_mime_types,
            with_first_line_pattern: self.with_first_line_pattern + other.with_first_line_pattern,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "{} languages, {} extensions, {} aliases, {} mime types, {} with first-line patterns",
            self.total,
            self.total_extensions,
            self.total_aliases,
            self.total_mime_types,
            self.with_first_line_pattern,
        )
    }
}

impl fmt::Display for LanguageStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ---------------------------------------------------------------------------
// LanguageFeatureSupport extensions
// ---------------------------------------------------------------------------

impl LanguageFeatureSupport {
    pub fn is_complete(&self) -> bool {
        self.completion
            && self.hover
            && self.formatting
            && self.diagnostics
            && self.go_to_definition
            && self.references
            && self.rename
            && self.code_actions
    }

    pub fn missing_features(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !self.completion { missing.push("completion"); }
        if !self.hover { missing.push("hover"); }
        if !self.formatting { missing.push("formatting"); }
        if !self.diagnostics { missing.push("diagnostics"); }
        if !self.go_to_definition { missing.push("go_to_definition"); }
        if !self.references { missing.push("references"); }
        if !self.rename { missing.push("rename"); }
        if !self.code_actions { missing.push("code_actions"); }
        missing
    }
}

// ---------------------------------------------------------------------------
// Syntax token classification
// ---------------------------------------------------------------------------

/// Classification of syntax tokens for semantic highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxTokenKind {
    Keyword,
    Identifier,
    StringLiteral,
    NumericLiteral,
    Comment,
    Operator,
    Punctuation,
    TypeName,
    FunctionName,
    Variable,
    Whitespace,
    Unknown,
}

impl SyntaxTokenKind {
    /// Returns `true` if the token carries semantic meaning.
    pub fn is_semantic(&self) -> bool {
        !matches!(self, Self::Whitespace | Self::Unknown | Self::Punctuation)
    }

    /// Returns a human-readable label for this token kind.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Identifier => "identifier",
            Self::StringLiteral => "string",
            Self::NumericLiteral => "number",
            Self::Comment => "comment",
            Self::Operator => "operator",
            Self::Punctuation => "punctuation",
            Self::TypeName => "type",
            Self::FunctionName => "function",
            Self::Variable => "variable",
            Self::Whitespace => "whitespace",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for SyntaxTokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------
// Language configuration with merging
// ---------------------------------------------------------------------------

/// Language-specific editor settings that can be layered and merged.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageConfiguration {
    pub language_id: String,
    pub tab_size: Option<u8>,
    pub insert_spaces: Option<bool>,
    pub word_wrap: Option<bool>,
    pub rulers: Vec<usize>,
    pub comment_line: Option<String>,
    pub comment_block_start: Option<String>,
    pub comment_block_end: Option<String>,
}

impl LanguageConfiguration {
    pub fn new(language_id: impl Into<String>) -> Self {
        Self {
            language_id: language_id.into(),
            tab_size: None,
            insert_spaces: None,
            word_wrap: None,
            rulers: Vec::new(),
            comment_line: None,
            comment_block_start: None,
            comment_block_end: None,
        }
    }

    /// Merges `other` into `self`, where `other` values take precedence when present.
    pub fn merge_from(&mut self, other: &LanguageConfiguration) {
        if other.tab_size.is_some() {
            self.tab_size = other.tab_size;
        }
        if other.insert_spaces.is_some() {
            self.insert_spaces = other.insert_spaces;
        }
        if other.word_wrap.is_some() {
            self.word_wrap = other.word_wrap;
        }
        if !other.rulers.is_empty() {
            self.rulers = other.rulers.clone();
        }
        if other.comment_line.is_some() {
            self.comment_line = other.comment_line.clone();
        }
        if other.comment_block_start.is_some() {
            self.comment_block_start = other.comment_block_start.clone();
        }
        if other.comment_block_end.is_some() {
            self.comment_block_end = other.comment_block_end.clone();
        }
    }

    /// Returns `true` if the configuration has block comment delimiters.
    pub fn has_block_comments(&self) -> bool {
        self.comment_block_start.is_some() && self.comment_block_end.is_some()
    }

    /// Returns the effective tab size, falling back to a default of 4.
    pub fn effective_tab_size(&self) -> u8 {
        self.tab_size.unwrap_or(4)
    }
}

// ---------------------------------------------------------------------------
// Embedded / multi-language document support
// ---------------------------------------------------------------------------

/// A region within a document that uses a different language.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddedLanguageRegion {
    pub language_id: String,
    pub start_line: usize,
    pub end_line: usize,
}

impl EmbeddedLanguageRegion {
    pub fn new(language_id: impl Into<String>, start_line: usize, end_line: usize) -> Self {
        Self {
            language_id: language_id.into(),
            start_line,
            end_line,
        }
    }

    /// Number of lines spanned by this region.
    pub fn line_count(&self) -> usize {
        if self.end_line >= self.start_line {
            self.end_line - self.start_line + 1
        } else {
            0
        }
    }

    /// Returns `true` if the given line falls within this region.
    pub fn contains_line(&self, line: usize) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

/// A document that tracks its primary language and any embedded language regions.
#[derive(Debug, Clone)]
pub struct MultiLanguageDocument {
    pub primary_language_id: String,
    pub regions: Vec<EmbeddedLanguageRegion>,
}

impl MultiLanguageDocument {
    pub fn new(primary_language_id: impl Into<String>) -> Self {
        Self {
            primary_language_id: primary_language_id.into(),
            regions: Vec::new(),
        }
    }

    pub fn add_region(&mut self, region: EmbeddedLanguageRegion) {
        self.regions.push(region);
    }

    /// Returns the language id for a given line number.
    pub fn language_at_line(&self, line: usize) -> &str {
        for region in &self.regions {
            if region.contains_line(line) {
                return &region.language_id;
            }
        }
        &self.primary_language_id
    }

    /// Returns all unique language ids present in the document.
    pub fn all_languages(&self) -> Vec<&str> {
        let mut langs: Vec<&str> = vec![&self.primary_language_id];
        for region in &self.regions {
            if !langs.contains(&region.language_id.as_str()) {
                langs.push(&region.language_id);
            }
        }
        langs
    }
}

// ---------------------------------------------------------------------------
// File association scoring
// ---------------------------------------------------------------------------

/// Scores how well a file matches a language, considering multiple signals.
#[derive(Debug, Clone, PartialEq)]
pub struct FileAssociationScore {
    pub language_id: String,
    pub extension_match: bool,
    pub filename_match: bool,
    pub first_line_match: bool,
    pub mime_match: bool,
}

impl FileAssociationScore {
    /// Computes a numeric score: each signal contributes a weight.
    pub fn score(&self) -> u32 {
        let mut s = 0u32;
        if self.extension_match {
            s += 10;
        }
        if self.filename_match {
            s += 5;
        }
        if self.first_line_match {
            s += 20;
        }
        if self.mime_match {
            s += 8;
        }
        s
    }
}

/// Scores all languages in a registry against a filename and optional first line.
pub fn score_languages(
    registry: &LanguageRegistry,
    filename: &str,
    first_line: Option<&str>,
) -> Vec<FileAssociationScore> {
    let mut scores: Vec<FileAssociationScore> = registry
        .languages
        .iter()
        .map(|lang| {
            let extension_match = lang.matches_filename(filename);
            let filename_match = lang.extensions.iter().any(|ext| {
                filename
                    .rsplit('/')
                    .next()
                    .map_or(false, |base| base == ext.trim_start_matches('.'))
            });
            let first_line_match = first_line.map_or(false, |fl| {
                lang.first_line_pattern
                    .as_ref()
                    .map_or(false, |pat| fl.contains(pat.as_str()))
            });
            FileAssociationScore {
                language_id: lang.id.clone(),
                extension_match,
                filename_match,
                first_line_match,
                mime_match: false,
            }
        })
        .filter(|s| s.score() > 0)
        .collect();
    scores.sort_by(|a, b| b.score().cmp(&a.score()));
    scores
}

// ---------------------------------------------------------------------------
// Shebang parser
// ---------------------------------------------------------------------------

/// Extracts the interpreter name from a shebang line (e.g. `#!/usr/bin/env python3` → `"python3"`).
pub fn parse_shebang(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if !trimmed.starts_with("#!") {
        return None;
    }
    let after_hash = trimmed.trim_start_matches("#!").trim();
    // Handle `env` wrapper: `#!/usr/bin/env python3`
    let parts: Vec<&str> = after_hash.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }
    let first = parts[0].rsplit('/').next().unwrap_or(parts[0]);
    if first == "env" {
        parts.get(1).map(|s| s.rsplit('/').next().unwrap_or(s))
    } else {
        Some(first)
    }
}

// ---------------------------------------------------------------------------
// LanguageModeSelector – UI logic for language mode selection
// ---------------------------------------------------------------------------

/// An entry in the language mode picker.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageModeEntry {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub is_configured: bool,
}

impl LanguageModeEntry {
    pub fn from_info(info: &LanguageInfo, is_configured: bool) -> Self {
        Self {
            id: info.id.clone(),
            name: info.name.clone(),
            aliases: info.aliases.clone(),
            is_configured,
        }
    }

    /// Match against a query string (case-insensitive, matches id, name, or aliases).
    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.id.to_lowercase().contains(&q)
            || self.name.to_lowercase().contains(&q)
            || self.aliases.iter().any(|a| a.to_lowercase().contains(&q))
    }
}

/// UI logic for filtering and ranking language mode entries.
pub struct LanguageModeSelector {
    entries: Vec<LanguageModeEntry>,
}

impl LanguageModeSelector {
    pub fn new(entries: Vec<LanguageModeEntry>) -> Self {
        Self { entries }
    }

    /// Filter entries by a query string.
    pub fn filter(&self, query: &str) -> Vec<&LanguageModeEntry> {
        if query.is_empty() {
            return self.entries.iter().collect();
        }
        self.entries.iter().filter(|e| e.matches_query(query)).collect()
    }

    /// Total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// LanguageFilenameAssociation – filename-pattern to language mapping
// ---------------------------------------------------------------------------

/// Associates filename patterns (globs) with a language.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageFilenameAssociation {
    pub pattern: String,
    pub language_id: String,
}

impl LanguageFilenameAssociation {
    pub fn new(pattern: impl Into<String>, language_id: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            language_id: language_id.into(),
        }
    }

    /// Check if a filename matches this association pattern.
    pub fn matches(&self, filename: &str) -> bool {
        if self.pattern.starts_with("*.") {
            let ext = &self.pattern[1..];
            filename.ends_with(ext)
        } else {
            filename == self.pattern
        }
    }
}

/// Resolve a filename to a language ID using filename associations.
pub fn resolve_language_by_filename(associations: &[LanguageFilenameAssociation], filename: &str) -> Option<String> {
    associations.iter().find(|a| a.matches(filename)).map(|a| a.language_id.clone())
}

// ---------------------------------------------------------------------------
// LanguageConfigApplicator – applies language-specific settings
// ---------------------------------------------------------------------------

/// A language-specific setting override.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageSettingOverride {
    pub language_id: String,
    pub key: String,
    pub value: String,
}

/// Collects and applies language-specific setting overrides.
pub struct LanguageConfigApplicator {
    overrides: Vec<LanguageSettingOverride>,
}

impl LanguageConfigApplicator {
    pub fn new() -> Self {
        Self { overrides: Vec::new() }
    }

    pub fn add(&mut self, language_id: impl Into<String>, key: impl Into<String>, value: impl Into<String>) {
        self.overrides.push(LanguageSettingOverride {
            language_id: language_id.into(),
            key: key.into(),
            value: value.into(),
        });
    }

    /// Get all overrides for a given language.
    pub fn get_overrides(&self, language_id: &str) -> Vec<&LanguageSettingOverride> {
        self.overrides.iter().filter(|o| o.language_id == language_id).collect()
    }

    /// Get the value of a specific key for a language.
    pub fn get_value(&self, language_id: &str, key: &str) -> Option<&str> {
        self.overrides
            .iter()
            .find(|o| o.language_id == language_id && o.key == key)
            .map(|o| o.value.as_str())
    }

    pub fn len(&self) -> usize {
        self.overrides.len()
    }

    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }
}

impl Default for LanguageConfigApplicator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Language detection priority ordering
// ---------------------------------------------------------------------------

/// Source of a language detection match, ordered by priority (highest first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DetectionPriority {
    UserAssociation = 4,
    Extension = 3,
    Shebang = 2,
    FirstLinePattern = 1,
    MimeType = 0,
}

impl fmt::Display for DetectionPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserAssociation => write!(f, "User Association"),
            Self::Extension => write!(f, "Extension"),
            Self::Shebang => write!(f, "Shebang"),
            Self::FirstLinePattern => write!(f, "First Line Pattern"),
            Self::MimeType => write!(f, "MIME Type"),
        }
    }
}

/// A detection result with its source priority.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub language_id: String,
    pub priority: DetectionPriority,
}

/// Select the best detection result (highest priority wins).
pub fn best_detection(results: &[DetectionResult]) -> Option<&DetectionResult> {
    results.iter().max_by_key(|r| r.priority)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rust() -> LanguageInfo {
        LanguageInfo {
            id: "rust".into(),
            name: "Rust".into(),
            extensions: vec![".rs".into()],
            aliases: vec!["rs".into()],
            mime_types: vec!["text/x-rust".into()],
            first_line_pattern: None,
        }
    }

    fn sample_python() -> LanguageInfo {
        LanguageInfo {
            id: "python".into(),
            name: "Python".into(),
            extensions: vec![".py".into(), ".pyw".into()],
            aliases: vec!["py".into()],
            mime_types: vec!["text/x-python".into()],
            first_line_pattern: Some(r"^#!.*\bpython".into()),
        }
    }

    #[test]
    fn register_and_lookup_by_id() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        assert_eq!(reg.language_count(), 2);

        let rust = reg.get_language("rust").unwrap();
        assert_eq!(rust.name, "Rust");

        assert!(reg.get_language("unknown").is_none());
    }

    #[test]
    fn lookup_by_extension_and_mime() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        let lang = reg.get_by_extension(".py").unwrap();
        assert_eq!(lang.id, "python");

        let lang = reg.get_by_mime_type("text/x-rust").unwrap();
        assert_eq!(lang.id, "rust");

        assert!(reg.get_by_extension(".unknown").is_none());
    }

    #[test]
    fn get_all_ids_returns_registered_languages() {
        let mut reg = LanguageRegistry::new();
        assert!(reg.get_all_ids().is_empty());

        reg.register(sample_rust());
        reg.register(sample_python());

        let mut ids = reg.get_all_ids();
        ids.sort();
        assert_eq!(ids, vec!["python", "rust"]);
    }

    #[test]
    fn matches_filename_and_has_alias() {
        let rust = sample_rust();
        assert!(rust.matches_filename("main.rs"));
        assert!(!rust.matches_filename("main.py"));
        assert!(rust.has_alias("rs"));
        assert!(!rust.has_alias("rust"));
    }

    #[test]
    fn display_trait() {
        let rust = sample_rust();
        assert_eq!(format!("{rust}"), "Rust (rust)");
    }

    #[test]
    fn lookup_by_alias_and_filename() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        assert_eq!(reg.get_by_alias("py").unwrap().id, "python");
        assert!(reg.get_by_alias("unknown").is_none());

        assert_eq!(reg.get_by_filename("lib.rs").unwrap().id, "rust");
        assert_eq!(reg.get_by_filename("script.pyw").unwrap().id, "python");
        assert!(reg.get_by_filename("data.json").is_none());
    }

    #[test]
    fn get_by_first_line_basic_contains() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        let py = reg
            .get_by_first_line("#!/usr/bin/env python3 ^#!.*\\bpython")
            .unwrap();
        assert_eq!(py.id, "python");

        // Rust has no first_line_pattern
        assert!(reg.get_by_first_line("fn main()").is_none());
    }

    #[test]
    fn unregister_removes_language() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        assert!(reg.unregister("rust"));
        assert!(!reg.has_language("rust"));
        assert!(reg.has_language("python"));
        assert_eq!(reg.language_count(), 1);

        // Removing a non-existent language returns false
        assert!(!reg.unregister("rust"));
    }

    #[test]
    fn get_all_extensions_across_languages() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        let mut exts = reg.get_all_extensions();
        exts.sort();
        assert_eq!(exts, vec![".py", ".pyw", ".rs"]);
    }

    #[test]
    fn search_case_insensitive() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        let results = reg.search("RUST");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "rust");

        let results = reg.search("py");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "python");

        // query matching nothing
        assert!(reg.search("java").is_empty());
    }

    #[test]
    fn builder_creates_valid_language() {
        let info = LanguageInfoBuilder::new("go", "Go")
            .extension(".go")
            .alias("golang")
            .mime_type("text/x-go")
            .first_line_pattern("//go:")
            .build()
            .unwrap();

        assert_eq!(info.id, "go");
        assert_eq!(info.name, "Go");
        assert_eq!(info.extensions, vec![".go"]);
        assert_eq!(info.aliases, vec!["golang"]);
        assert_eq!(info.first_line_pattern.as_deref(), Some("//go:"));
    }

    #[test]
    fn builder_rejects_empty_id() {
        let err = LanguageInfoBuilder::new("", "Name")
            .build()
            .unwrap_err();
        assert_eq!(err, LanguageError::EmptyId);
    }

    #[test]
    fn builder_rejects_empty_name() {
        let err = LanguageInfoBuilder::new("id", "")
            .build()
            .unwrap_err();
        assert_eq!(err, LanguageError::EmptyName);
    }

    #[test]
    fn try_register_validates_and_rejects_duplicates() {
        let mut reg = LanguageRegistry::new();
        let rust = sample_rust();
        reg.try_register(rust.clone()).unwrap();
        assert_eq!(reg.language_count(), 1);

        let err = reg.try_register(rust).unwrap_err();
        assert_eq!(err, LanguageError::DuplicateId("rust".into()));
    }

    #[test]
    fn try_register_rejects_invalid_language() {
        let mut reg = LanguageRegistry::new();
        let bad = LanguageInfo {
            id: "".into(),
            name: "Bad".into(),
            extensions: vec![],
            aliases: vec![],
            mime_types: vec![],
            first_line_pattern: None,
        };
        assert_eq!(reg.try_register(bad).unwrap_err(), LanguageError::EmptyId);
    }

    #[test]
    fn require_language_returns_error_on_missing() {
        let reg = LanguageRegistry::new();
        let err = reg.require_language("go").unwrap_err();
        assert_eq!(err, LanguageError::NotFound("go".into()));
    }

    #[test]
    fn primary_extension_and_mime() {
        let rust = sample_rust();
        assert_eq!(rust.primary_extension(), Some(".rs"));
        assert_eq!(rust.primary_mime_type(), Some("text/x-rust"));
        assert!(rust.has_mime_types());

        let empty = LanguageInfo {
            id: "plain".into(),
            name: "Plain".into(),
            extensions: vec![],
            aliases: vec![],
            mime_types: vec![],
            first_line_pattern: None,
        };
        assert_eq!(empty.primary_extension(), None);
        assert!(!empty.has_mime_types());
    }

    #[test]
    fn metadata_count_sums_all_fields() {
        let py = sample_python();
        // 2 extensions + 1 alias + 1 mime = 4
        assert_eq!(py.metadata_count(), 4);
    }

    #[test]
    fn get_all_mime_types_deduplicates() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        let mimes = reg.get_all_mime_types();
        assert_eq!(mimes, vec!["text/x-python", "text/x-rust"]);
    }

    #[test]
    fn sorted_by_name_alphabetical() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        let sorted = reg.sorted_by_name();
        assert_eq!(sorted[0].id, "python");
        assert_eq!(sorted[1].id, "rust");
    }

    #[test]
    fn merge_skips_duplicates() {
        let mut reg1 = LanguageRegistry::new();
        reg1.register(sample_rust());

        let mut reg2 = LanguageRegistry::new();
        reg2.register(sample_rust());
        reg2.register(sample_python());

        reg1.merge(&reg2);
        assert_eq!(reg1.language_count(), 2);
        assert!(reg1.has_language("rust"));
        assert!(reg1.has_language("python"));
    }

    #[test]
    fn language_error_display() {
        assert_eq!(LanguageError::EmptyId.to_string(), "language id must not be empty");
        assert_eq!(
            LanguageError::DuplicateId("rs".into()).to_string(),
            "language 'rs' is already registered"
        );
        assert_eq!(
            LanguageError::NotFound("x".into()).to_string(),
            "no language found for 'x'"
        );
    }

    #[test]
    fn language_info_partial_eq() {
        let a = sample_rust();
        let b = sample_rust();
        assert_eq!(a, b);

        let c = sample_python();
        assert_ne!(a, c);
    }

    #[test]
    fn stats_empty_registry() {
        let stats = compute_language_status_stats(&[]);
        assert_eq!(
            stats,
            LanguageStatusStats {
                total_items: 0,
                active_count: 0,
                error_count: 0,
            }
        );
    }

    #[test]
    fn stats_all_active() {
        let langs = vec![sample_rust(), sample_python()];
        let stats = compute_language_status_stats(&langs);
        assert_eq!(stats.total_items, 2);
        assert_eq!(stats.active_count, 2);
        assert_eq!(stats.error_count, 0);
    }

    #[test]
    fn stats_counts_errors() {
        let bad = LanguageInfo {
            id: "".into(),
            name: "Bad".into(),
            extensions: vec![".bad".into()],
            aliases: vec![],
            mime_types: vec![],
            first_line_pattern: None,
        };
        let langs = vec![sample_rust(), bad];
        let stats = compute_language_status_stats(&langs);
        assert_eq!(stats.total_items, 2);
        assert_eq!(stats.active_count, 1);
        assert_eq!(stats.error_count, 1);
    }

    #[test]
    fn stats_inactive_language_without_extensions() {
        let no_ext = LanguageInfo {
            id: "plain".into(),
            name: "Plaintext".into(),
            extensions: vec![],
            aliases: vec![],
            mime_types: vec!["text/plain".into()],
            first_line_pattern: None,
        };
        let langs = vec![sample_rust(), no_ext];
        let stats = compute_language_status_stats(&langs);
        assert_eq!(stats.total_items, 2);
        assert_eq!(stats.active_count, 1);
        assert_eq!(stats.error_count, 0);
    }

    #[test]
    fn stats_display_format() {
        let stats = LanguageStatusStats {
            total_items: 5,
            active_count: 3,
            error_count: 1,
        };
        assert_eq!(
            format!("{stats}"),
            "languages: 5 total, 3 active, 1 errors"
        );
    }

    #[test]
    fn stats_from_registry() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());
        let stats = compute_language_status_stats(&reg.languages);
        assert_eq!(stats.total_items, reg.language_count());
        assert_eq!(stats.active_count, 2);
    }

    #[test]
    fn language_statistics_computation() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());
        let stats = compute_language_statistics(&reg);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.total_extensions, 3); // .rs + .py + .pyw
        assert_eq!(stats.total_aliases, 2); // rs + py
        assert_eq!(stats.total_mime_types, 2);
        assert_eq!(stats.with_first_line_pattern, 1); // only python
    }

    #[test]
    fn group_languages_by_initial_letter() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());
        let groups = group_languages_by_initial(&reg);
        assert_eq!(groups.len(), 2); // P and R
        assert_eq!(groups[0].category, "P");
        assert_eq!(groups[1].category, "R");
    }

    #[test]
    fn resolve_alias_finds_language() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());
        assert_eq!(resolve_alias(&reg, "rs"), Some("rust".to_string()));
        assert_eq!(resolve_alias(&reg, "py"), Some("python".to_string()));
        assert_eq!(resolve_alias(&reg, "unknown"), None);
    }

    #[test]
    fn feature_matrix_new_defaults() {
        let fm = LanguageFeatureMatrix::new("rust");
        assert_eq!(fm.language_id, "rust");
        assert!(!fm.has_folding);
        assert_eq!(fm.enabled_count(), 0);
    }

    #[test]
    fn feature_matrix_enabled_count() {
        let fm = LanguageFeatureMatrix {
            language_id: "rust".into(),
            has_folding: true,
            has_indentation_rules: true,
            has_bracket_pairs: false,
            has_auto_closing_pairs: true,
        };
        assert_eq!(fm.enabled_count(), 3);
    }

    // -- LanguageAssociation tests ----------------------------------------

    #[test]
    fn resolve_language_by_extension_basic() {
        let assocs = vec![
            LanguageAssociation::new(".rs", "rust"),
            LanguageAssociation::new(".py", "python"),
        ];
        assert_eq!(
            resolve_language_by_extension(&assocs, "main.rs"),
            Some("rust".into())
        );
    }

    #[test]
    fn resolve_language_by_extension_case_insensitive() {
        let assocs = vec![LanguageAssociation::new(".RS", "rust")];
        assert_eq!(
            resolve_language_by_extension(&assocs, "main.rs"),
            Some("rust".into())
        );
    }

    #[test]
    fn resolve_language_by_extension_no_match() {
        let assocs = vec![LanguageAssociation::new(".rs", "rust")];
        assert_eq!(
            resolve_language_by_extension(&assocs, "main.py"),
            None
        );
    }

    #[test]
    fn resolve_language_by_extension_first_wins() {
        let assocs = vec![
            LanguageAssociation::new(".ts", "typescript"),
            LanguageAssociation::new(".ts", "typescriptreact"),
        ];
        assert_eq!(
            resolve_language_by_extension(&assocs, "app.ts"),
            Some("typescript".into())
        );
    }

    #[test]
    fn language_association_new() {
        let a = LanguageAssociation::new(".go", "go");
        assert_eq!(a.extension, ".go");
        assert_eq!(a.language_id, "go");
    }

    // -- LanguageDetector tests --------------------------------------------

    #[test]
    fn detector_shebang_python() {
        let d = LanguageDetector::new();
        assert_eq!(
            d.detect_from_first_line("#!/usr/bin/env python3"),
            Some("python".into())
        );
    }

    #[test]
    fn detector_shebang_bash() {
        let d = LanguageDetector::new();
        assert_eq!(
            d.detect_from_first_line("#!/bin/bash"),
            Some("shellscript".into())
        );
    }

    #[test]
    fn detector_shebang_node() {
        let d = LanguageDetector::new();
        assert_eq!(
            d.detect_from_first_line("#!/usr/bin/env node"),
            Some("javascript".into())
        );
    }

    #[test]
    fn detector_xml_declaration() {
        let d = LanguageDetector::new();
        assert_eq!(
            d.detect_from_first_line("<?xml version=\"1.0\"?>"),
            Some("xml".into())
        );
    }

    #[test]
    fn detector_html_doctype() {
        let d = LanguageDetector::new();
        assert_eq!(
            d.detect_from_first_line("<!DOCTYPE html>"),
            Some("html".into())
        );
    }

    #[test]
    fn detector_json_brace() {
        let d = LanguageDetector::new();
        assert_eq!(
            d.detect_from_first_line("{"),
            Some("json".into())
        );
    }

    #[test]
    fn detector_no_match() {
        let d = LanguageDetector::new();
        assert_eq!(d.detect_from_first_line("fn main() {}"), None);
    }

    #[test]
    fn detector_from_content() {
        let d = LanguageDetector::new();
        let content = "#!/bin/bash\necho hello\n";
        assert_eq!(d.detect_from_content(content), Some("shellscript".into()));
    }

    #[test]
    fn detector_from_empty_content() {
        let d = LanguageDetector::new();
        assert_eq!(d.detect_from_content(""), None);
    }

    // -- language_overlap tests --------------------------------------------

    #[test]
    fn overlap_finds_shared_extensions() {
        let assocs = vec![
            LanguageAssociation::new(".ts", "typescript"),
            LanguageAssociation::new(".ts", "typescriptreact"),
            LanguageAssociation::new(".rs", "rust"),
        ];
        let overlaps = language_overlap(&assocs);
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].0, ".ts");
        assert_eq!(overlaps[0].1, vec!["typescript", "typescriptreact"]);
    }

    #[test]
    fn overlap_no_duplicates() {
        let assocs = vec![
            LanguageAssociation::new(".rs", "rust"),
            LanguageAssociation::new(".py", "python"),
        ];
        let overlaps = language_overlap(&assocs);
        assert!(overlaps.is_empty());
    }

    // -- LanguageFeatureSupport tests --------------------------------------

    #[test]
    fn feature_support_default_zero() {
        let fs = LanguageFeatureSupport::default();
        assert_eq!(fs.supports_count(), 0);
    }

    #[test]
    fn feature_support_counts_enabled() {
        let fs = LanguageFeatureSupport {
            completion: true,
            hover: true,
            formatting: false,
            diagnostics: true,
            go_to_definition: false,
            references: false,
            rename: true,
            code_actions: false,
        };
        assert_eq!(fs.supports_count(), 4);
    }

    #[test]
    fn feature_support_all_enabled() {
        let fs = LanguageFeatureSupport {
            completion: true,
            hover: true,
            formatting: true,
            diagnostics: true,
            go_to_definition: true,
            references: true,
            rename: true,
            code_actions: true,
        };
        assert_eq!(fs.supports_count(), 8);
    }

    #[test]
    fn language_info_filter_and_counts() {
        let py = sample_python();
        assert!(py.has_aliases());
        assert_eq!(py.alias_count(), 1);
        assert_eq!(py.extension_count(), 2);
        assert!(py.matches_filter("pyth"));
        assert!(py.matches_filter(".py"));
        assert!(!py.matches_filter("java"));

        let empty = LanguageInfo {
            id: "plain".into(),
            name: "Plaintext".into(),
            extensions: vec![],
            aliases: vec![],
            mime_types: vec![],
            first_line_pattern: None,
        };
        assert!(!empty.has_aliases());
        assert_eq!(empty.alias_count(), 0);
    }

    #[test]
    fn registry_iter_and_into_iterator() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        let ids_iter: Vec<&str> = reg.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids_iter.len(), 2);

        let ids_into: Vec<&str> = (&reg).into_iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids_into, ids_iter);
    }

    #[test]
    fn registry_all_extensions_deduped() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());
        let exts = reg.all_extensions();
        assert_eq!(exts, vec![".py", ".pyw", ".rs"]);
    }

    #[test]
    fn registry_find_by_alias_case_insensitive() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());
        let found = reg.find_by_alias("RS");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "rust");
        assert!(reg.find_by_alias("unknown").is_empty());
    }

    #[test]
    fn language_group_extensions_and_display() {
        let group = LanguageGroup {
            category: "R".into(),
            language_ids: vec!["rust".into(), "ruby".into()],
        };
        assert_eq!(group.total_languages(), 2);
        assert!(!group.is_empty());
        assert_eq!(format!("{group}"), "R (2 languages)");

        let empty_group = LanguageGroup {
            category: "Z".into(),
            language_ids: vec![],
        };
        assert!(empty_group.is_empty());
    }

    #[test]
    fn feature_matrix_extensions() {
        let fm = LanguageFeatureMatrix {
            language_id: "rust".into(),
            has_folding: true,
            has_indentation_rules: false,
            has_bracket_pairs: true,
            has_auto_closing_pairs: true,
        };
        let supported = fm.supported_features();
        assert_eq!(supported, vec!["folding", "bracket_pairs", "auto_closing_pairs"]);
        assert_eq!(fm.feature_count(), 4);
        assert!(!fm.is_fully_supported());

        let full = LanguageFeatureMatrix {
            language_id: "go".into(),
            has_folding: true,
            has_indentation_rules: true,
            has_bracket_pairs: true,
            has_auto_closing_pairs: true,
        };
        assert!(full.is_fully_supported());
    }

    #[test]
    fn association_display_and_extension_based() {
        let a = LanguageAssociation::new(".rs", "rust");
        assert_eq!(format!("{a}"), ".rs -> rust");
        assert!(a.is_extension_based());

        let b = LanguageAssociation::new("Makefile", "makefile");
        assert!(!b.is_extension_based());
    }

    #[test]
    fn detector_add_rule_and_count() {
        let mut d = LanguageDetector::new();
        let initial = d.rule_count();
        d.add_rule("#!/usr/bin/env ruby", "ruby");
        assert_eq!(d.rule_count(), initial + 1);
        assert_eq!(
            d.detect_from_first_line("#!/usr/bin/env ruby"),
            Some("ruby".into())
        );
    }

    #[test]
    fn statistics_merge_and_summary() {
        let mut reg1 = LanguageRegistry::new();
        reg1.register(sample_rust());
        let stats1 = compute_language_statistics(&reg1);

        let mut reg2 = LanguageRegistry::new();
        reg2.register(sample_python());
        let stats2 = compute_language_statistics(&reg2);

        let merged = stats1.merge(&stats2);
        assert_eq!(merged.total, 2);
        assert_eq!(merged.total_extensions, 3);
        assert_eq!(merged.total_aliases, 2);
        assert!(merged.summary().contains("2 languages"));
        assert_eq!(format!("{merged}"), merged.summary());
    }

    #[test]
    fn feature_support_complete_and_missing() {
        let full = LanguageFeatureSupport {
            completion: true,
            hover: true,
            formatting: true,
            diagnostics: true,
            go_to_definition: true,
            references: true,
            rename: true,
            code_actions: true,
        };
        assert!(full.is_complete());
        assert!(full.missing_features().is_empty());

        let partial = LanguageFeatureSupport {
            completion: true,
            hover: false,
            formatting: true,
            diagnostics: false,
            go_to_definition: true,
            references: false,
            rename: true,
            code_actions: false,
        };
        assert!(!partial.is_complete());
        assert_eq!(
            partial.missing_features(),
            vec!["hover", "diagnostics", "references", "code_actions"]
        );
    }

    // -----------------------------------------------------------------------
    // New tests: syntax token classification
    // -----------------------------------------------------------------------

    #[test]
    fn syntax_token_kind_semantic_and_label() {
        assert!(SyntaxTokenKind::Keyword.is_semantic());
        assert!(SyntaxTokenKind::FunctionName.is_semantic());
        assert!(!SyntaxTokenKind::Whitespace.is_semantic());
        assert!(!SyntaxTokenKind::Unknown.is_semantic());
        assert!(!SyntaxTokenKind::Punctuation.is_semantic());

        assert_eq!(SyntaxTokenKind::Comment.label(), "comment");
        assert_eq!(SyntaxTokenKind::Operator.label(), "operator");
        assert_eq!(format!("{}", SyntaxTokenKind::TypeName), "type");
    }

    // -----------------------------------------------------------------------
    // New tests: language configuration merging
    // -----------------------------------------------------------------------

    #[test]
    fn language_configuration_merge_and_defaults() {
        let mut base = LanguageConfiguration::new("rust");
        base.tab_size = Some(4);
        base.insert_spaces = Some(true);
        base.comment_line = Some("//".into());

        let mut overlay = LanguageConfiguration::new("rust");
        overlay.tab_size = Some(2);
        overlay.rulers = vec![80, 120];
        overlay.comment_block_start = Some("/*".into());
        overlay.comment_block_end = Some("*/".into());

        base.merge_from(&overlay);

        assert_eq!(base.tab_size, Some(2)); // overridden
        assert_eq!(base.insert_spaces, Some(true)); // kept
        assert_eq!(base.rulers, vec![80, 120]); // replaced
        assert_eq!(base.comment_line.as_deref(), Some("//")); // kept
        assert!(base.has_block_comments());
        assert_eq!(base.effective_tab_size(), 2);

        let empty = LanguageConfiguration::new("go");
        assert_eq!(empty.effective_tab_size(), 4); // default
        assert!(!empty.has_block_comments());
    }

    // -----------------------------------------------------------------------
    // New tests: multi-language document
    // -----------------------------------------------------------------------

    #[test]
    fn multi_language_document_regions() {
        let mut doc = MultiLanguageDocument::new("html");
        doc.add_region(EmbeddedLanguageRegion::new("css", 5, 15));
        doc.add_region(EmbeddedLanguageRegion::new("javascript", 20, 30));

        assert_eq!(doc.language_at_line(0), "html");
        assert_eq!(doc.language_at_line(5), "css");
        assert_eq!(doc.language_at_line(10), "css");
        assert_eq!(doc.language_at_line(18), "html");
        assert_eq!(doc.language_at_line(25), "javascript");

        let langs = doc.all_languages();
        assert_eq!(langs, vec!["html", "css", "javascript"]);

        let region = &doc.regions[0];
        assert_eq!(region.line_count(), 11);
        assert!(region.contains_line(5));
        assert!(region.contains_line(15));
        assert!(!region.contains_line(16));
    }

    // -----------------------------------------------------------------------
    // New tests: file association scoring
    // -----------------------------------------------------------------------

    #[test]
    fn score_languages_ranks_by_signals() {
        let mut reg = LanguageRegistry::new();
        reg.register(sample_rust());
        reg.register(sample_python());

        let scores = score_languages(&reg, "main.rs", None);
        assert!(!scores.is_empty());
        assert_eq!(scores[0].language_id, "rust");
        assert!(scores[0].extension_match);
        assert_eq!(scores[0].score(), 10);

        // With first-line match for python
        let scores2 = score_languages(
            &reg,
            "script",
            Some("^#!.*\\bpython something"),
        );
        assert_eq!(scores2[0].language_id, "python");
        assert!(scores2[0].first_line_match);
        assert!(scores2[0].score() >= 20);
    }

    // -----------------------------------------------------------------------
    // New tests: shebang parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_shebang_extracts_interpreter() {
        assert_eq!(parse_shebang("#!/usr/bin/env python3"), Some("python3"));
        assert_eq!(parse_shebang("#!/bin/bash"), Some("bash"));
        assert_eq!(parse_shebang("#!/usr/bin/env node"), Some("node"));
        assert_eq!(parse_shebang("#!/usr/local/bin/ruby"), Some("ruby"));
        assert_eq!(parse_shebang("  #!/usr/bin/env perl  "), Some("perl"));
        assert_eq!(parse_shebang("no shebang here"), None);
        assert_eq!(parse_shebang(""), None);
    }

    // -- LanguageModeSelector tests --

    #[test]
    fn mode_entry_matches_query() {
        let entry = LanguageModeEntry {
            id: "rust".into(),
            name: "Rust".into(),
            aliases: vec!["rs".into()],
            is_configured: false,
        };
        assert!(entry.matches_query("rust"));
        assert!(entry.matches_query("Ru"));
        assert!(entry.matches_query("rs"));
        assert!(!entry.matches_query("python"));
    }

    #[test]
    fn mode_selector_filter() {
        let entries = vec![
            LanguageModeEntry { id: "rust".into(), name: "Rust".into(), aliases: vec![], is_configured: true },
            LanguageModeEntry { id: "python".into(), name: "Python".into(), aliases: vec!["py".into()], is_configured: false },
        ];
        let selector = LanguageModeSelector::new(entries);
        assert_eq!(selector.len(), 2);
        let filtered = selector.filter("py");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "python");
    }

    #[test]
    fn mode_selector_empty_query() {
        let selector = LanguageModeSelector::new(vec![
            LanguageModeEntry { id: "a".into(), name: "A".into(), aliases: vec![], is_configured: false },
        ]);
        assert_eq!(selector.filter("").len(), 1);
    }

    // -- LanguageFilenameAssociation tests --

    #[test]
    fn association_matches_extension() {
        let assoc = LanguageFilenameAssociation::new("*.rs", "rust");
        assert!(assoc.matches("main.rs"));
        assert!(!assoc.matches("main.py"));
    }

    #[test]
    fn association_matches_filename() {
        let assoc = LanguageFilenameAssociation::new("Makefile", "makefile");
        assert!(assoc.matches("Makefile"));
        assert!(!assoc.matches("makefile"));
    }

    #[test]
    fn resolve_language_from_filename_associations() {
        let assocs = vec![
            LanguageFilenameAssociation::new("*.rs", "rust"),
            LanguageFilenameAssociation::new("*.py", "python"),
        ];
        assert_eq!(resolve_language_by_filename(&assocs, "lib.rs"), Some("rust".into()));
        assert_eq!(resolve_language_by_filename(&assocs, "unknown.txt"), None);
    }

    // -- LanguageConfigApplicator tests --

    #[test]
    fn config_applicator_basic() {
        let mut app = LanguageConfigApplicator::new();
        app.add("rust", "editor.tabSize", "4");
        app.add("rust", "editor.formatOnSave", "true");
        app.add("python", "editor.tabSize", "2");
        let overrides = app.get_overrides("rust");
        assert_eq!(overrides.len(), 2);
        assert_eq!(app.get_value("rust", "editor.tabSize"), Some("4"));
        assert_eq!(app.get_value("python", "editor.tabSize"), Some("2"));
        assert_eq!(app.get_value("go", "editor.tabSize"), None);
    }

    #[test]
    fn config_applicator_empty() {
        let app = LanguageConfigApplicator::default();
        assert!(app.is_empty());
    }

    // -- Detection priority tests --

    #[test]
    fn detection_priority_ordering() {
        assert!(DetectionPriority::UserAssociation > DetectionPriority::Extension);
        assert!(DetectionPriority::Extension > DetectionPriority::Shebang);
        assert!(DetectionPriority::Shebang > DetectionPriority::FirstLinePattern);
    }

    #[test]
    fn best_detection_highest_wins() {
        let results = vec![
            DetectionResult { language_id: "python".into(), priority: DetectionPriority::Shebang },
            DetectionResult { language_id: "bash".into(), priority: DetectionPriority::Extension },
        ];
        let best = best_detection(&results).unwrap();
        assert_eq!(best.language_id, "bash");
    }

    #[test]
    fn best_detection_empty() {
        assert!(best_detection(&[]).is_none());
    }

    #[test]
    fn detection_priority_display() {
        assert_eq!(format!("{}", DetectionPriority::Shebang), "Shebang");
    }
}
