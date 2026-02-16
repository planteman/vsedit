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
}
