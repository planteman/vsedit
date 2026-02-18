//! Language registration, detection, and editing configuration.
//!
//! Manages language definitions and provides lookup by file extension,
//! filename, MIME type, and first-line content (e.g. shebangs). Also provides
//! per-language editing behaviour: commenting, auto-closing pairs, brackets,
//! folding markers, indentation rules, and word patterns.
//!
//! # Key types
//!
//! - [`LanguageDefinition`] — describes a single language for registration.
//! - [`LanguageConfiguration`] — per-language editing behaviour.
//! - [`LanguageService`] — stores languages and provides fast lookups.
//! - [`register_default_languages`] — registers 30+ common languages.

use std::fmt;
mod config;
mod defaults;
mod definition;
mod editing;
mod registry;

pub use config::*;
pub use defaults::register_default_languages;
pub use definition::*;
pub use editing::*;
pub use registry::*;

// Re-export the old name as an alias for backward compatibility.
pub type LanguageConfiguration = LanguageDefinition;
pub type LanguageRegistry = LanguageService;

/// A language identifier string (e.g. `"rust"`, `"typescript"`).
pub type LanguageId = String;

// ---------------------------------------------------------------------------
// Language statistics
// ---------------------------------------------------------------------------

/// Aggregate statistics about the languages registered in a [`LanguageService`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageStats {
    /// Total number of registered languages.
    pub total_languages: usize,
    /// Total number of file extensions across all languages.
    pub total_extensions: usize,
    /// Number of languages that declare a first-line / shebang pattern.
    pub languages_with_shebangs: usize,
    /// Number of languages whose edit config includes block comments.
    pub languages_with_block_comments: usize,
}

/// Compute aggregate statistics for all languages in a [`LanguageService`].
pub fn compute_language_stats(svc: &LanguageService) -> LanguageStats {
    let ids = svc.get_registered_language_ids();
    let total_languages = ids.len();

    let mut total_extensions: usize = 0;
    let mut languages_with_shebangs: usize = 0;
    let mut languages_with_block_comments: usize = 0;

    for id in &ids {
        if let Some(lang) = svc.get_language(id) {
            total_extensions += lang.extensions.len();
            if lang.first_line.is_some() {
                languages_with_shebangs += 1;
            }
        }
        if let Some(cfg) = svc.get_edit_config(id) {
            if cfg.comments.block_comment.is_some() {
                languages_with_block_comments += 1;
            }
        }
    }

    LanguageStats {
        total_languages,
        total_extensions,
        languages_with_shebangs,
        languages_with_block_comments,
    }
}

// ---------------------------------------------------------------------------
// Language similarity
// ---------------------------------------------------------------------------

/// Describes how similar another language is to a reference language based on
/// the number of shared file extensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageSimilarity {
    /// The id of the similar language.
    pub language_id: String,
    /// File extensions shared between the two languages.
    pub shared_extensions: Vec<String>,
}

/// Find languages that share at least one file extension with `lang_id`.
///
/// Results are sorted by number of shared extensions (descending).  The
/// queried language itself is never included.
pub fn find_similar_languages(svc: &LanguageService, lang_id: &str) -> Vec<LanguageSimilarity> {
    let target = match svc.get_language(lang_id) {
        Some(l) => l,
        None => return Vec::new(),
    };

    let target_exts: std::collections::HashSet<String> =
        target.extensions.iter().map(|e| e.to_lowercase()).collect();

    let mut results: Vec<LanguageSimilarity> = Vec::new();

    for other_id in svc.get_registered_language_ids() {
        if other_id == lang_id {
            continue;
        }
        if let Some(other) = svc.get_language(other_id) {
            let shared: Vec<String> = other
                .extensions
                .iter()
                .filter(|e| target_exts.contains(&e.to_lowercase()))
                .cloned()
                .collect();
            if !shared.is_empty() {
                results.push(LanguageSimilarity {
                    language_id: other_id.to_string(),
                    shared_extensions: shared,
                });
            }
        }
    }

    results.sort_by(|a, b| b.shared_extensions.len().cmp(&a.shared_extensions.len()));
    results
}

// ---------------------------------------------------------------------------
// Bulk detection
// ---------------------------------------------------------------------------

/// Detect the language for each filename in `filenames`.
///
/// Returns a `Vec` of `(filename, language_id)` pairs. When the language
/// cannot be determined, the second element is `None`.
pub fn bulk_detect<'a>(
    svc: &'a LanguageService,
    filenames: &[&str],
) -> Vec<(String, Option<&'a str>)> {
    filenames
        .iter()
        .map(|name| {
            let id = svc.guess_language_id(name, None);
            ((*name).to_string(), id)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Language contribution scoring
// ---------------------------------------------------------------------------

/// Score representing how strongly a language matches a given file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageMatchScore {
    pub language_id: String,
    pub extension_match: bool,
    pub filename_match: bool,
    pub first_line_match: bool,
}

impl LanguageMatchScore {
    /// Compute a numeric score (higher = better match).
    pub fn score(&self) -> u32 {
        let mut s = 0u32;
        if self.filename_match {
            s += 10;
        }
        if self.extension_match {
            s += 5;
        }
        if self.first_line_match {
            s += 3;
        }
        s
    }
}

/// Score all registered languages against a filename and optional first line.
pub fn score_languages(svc: &LanguageService, filename: &str, first_line: Option<&str>) -> Vec<LanguageMatchScore> {
    let basename = filename.rsplit('/').next().unwrap_or(filename);
    let ext = basename.rfind('.').map(|i| &basename[i..]);

    let mut scores = Vec::new();
    for id in svc.get_registered_language_ids() {
        if let Some(lang) = svc.get_language(id) {
            let filename_match = lang.filenames.iter().any(|f| f.eq_ignore_ascii_case(basename));
            let extension_match = ext.map_or(false, |e| {
                lang.extensions.iter().any(|le| le.eq_ignore_ascii_case(e))
            });
            let first_line_match = first_line.map_or(false, |fl| {
                svc.get_language_id_by_first_line(fl) == Some(id)
            });
            if filename_match || extension_match || first_line_match {
                scores.push(LanguageMatchScore {
                    language_id: id.to_string(),
                    extension_match,
                    filename_match,
                    first_line_match,
                });
            }
        }
    }
    scores.sort_by(|a, b| b.score().cmp(&a.score()));
    scores
}

// ---------------------------------------------------------------------------
// Extension-to-language conflict resolution
// ---------------------------------------------------------------------------

/// Describes a conflict where multiple languages claim the same extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionConflict {
    pub extension: String,
    pub language_ids: Vec<String>,
}

/// Find all file extensions that are registered by more than one language.
pub fn find_extension_conflicts(svc: &LanguageService) -> Vec<ExtensionConflict> {
    let mut ext_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for id in svc.get_registered_language_ids() {
        if let Some(lang) = svc.get_language(id) {
            for ext in &lang.extensions {
                let lower = ext.to_lowercase();
                ext_map.entry(lower).or_default().push(id.to_string());
            }
        }
    }
    let mut conflicts: Vec<ExtensionConflict> = ext_map
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(extension, language_ids)| ExtensionConflict { extension, language_ids })
        .collect();
    conflicts.sort_by(|a, b| a.extension.cmp(&b.extension));
    conflicts
}

// ---------------------------------------------------------------------------
// Shebang parsing enhancement
// ---------------------------------------------------------------------------

/// Parsed components of a shebang line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShebangInfo {
    pub interpreter_path: String,
    pub interpreter_name: String,
    pub args: Vec<String>,
}

/// Parse a shebang line into its components.
/// Returns `None` if the line doesn't start with `#!`.
pub fn parse_shebang(line: &str) -> Option<ShebangInfo> {
    let trimmed = line.trim();
    if !trimmed.starts_with("#!") {
        return None;
    }
    let rest = trimmed[2..].trim();
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    // Handle `#!/usr/bin/env <interpreter>` form
    let (interpreter_path, interpreter_name, args) = if parts[0].ends_with("/env") && parts.len() > 1 {
        let name = parts[1].to_string();
        let args: Vec<String> = parts[2..].iter().map(|s| s.to_string()).collect();
        (parts[0].to_string(), name, args)
    } else {
        let path = parts[0].to_string();
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        (path, name, args)
    };

    Some(ShebangInfo { interpreter_path, interpreter_name, args })
}

// ---------------------------------------------------------------------------
// Language inheritance chain
// ---------------------------------------------------------------------------

/// Describes the inheritance chain for a language based on shared editing config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageInheritance {
    pub language_id: String,
    pub parent_id: Option<String>,
    pub shared_comment_style: bool,
}

/// Determine potential parent languages based on shared comment styles.
pub fn find_language_parent(svc: &LanguageService, lang_id: &str) -> Option<LanguageInheritance> {
    let cfg = svc.get_edit_config(lang_id)?;
    let line_comment = &cfg.comments.line_comment;
    let block_comment = &cfg.comments.block_comment;

    for other_id in svc.get_registered_language_ids() {
        if other_id == lang_id {
            continue;
        }
        if let Some(other_cfg) = svc.get_edit_config(other_id) {
            if &other_cfg.comments.line_comment == line_comment
                && &other_cfg.comments.block_comment == block_comment
            {
                return Some(LanguageInheritance {
                    language_id: lang_id.to_string(),
                    parent_id: Some(other_id.to_string()),
                    shared_comment_style: true,
                });
            }
        }
    }

    Some(LanguageInheritance {
        language_id: lang_id.to_string(),
        parent_id: None,
        shared_comment_style: false,
    })
}

// ---------------------------------------------------------------------------
// Language Feature Registry
// ---------------------------------------------------------------------------

/// Features that a language can support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageFeature {
    Completion,
    Hover,
    SignatureHelp,
    Definition,
    References,
    DocumentHighlight,
    DocumentSymbol,
    CodeAction,
    Formatting,
    RangeFormatting,
    Rename,
    FoldingRange,
    SelectionRange,
    InlayHints,
}

/// Registry tracking which features are available for each language.
pub struct LanguageFeatureRegistry {
    features: std::collections::HashMap<String, std::collections::HashSet<LanguageFeature>>,
}

impl LanguageFeatureRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            features: std::collections::HashMap::new(),
        }
    }

    /// Register a feature for the given language.
    pub fn register_feature(&mut self, lang_id: &str, feature: LanguageFeature) {
        self.features
            .entry(lang_id.to_string())
            .or_default()
            .insert(feature);
    }

    /// Unregister a feature for the given language. Returns `true` if it was present.
    pub fn unregister_feature(&mut self, lang_id: &str, feature: &LanguageFeature) -> bool {
        self.features
            .get_mut(lang_id)
            .map_or(false, |set| set.remove(feature))
    }

    /// Check whether a language has a specific feature registered.
    pub fn has_feature(&self, lang_id: &str, feature: &LanguageFeature) -> bool {
        self.features
            .get(lang_id)
            .map_or(false, |set| set.contains(feature))
    }

    /// Return all features registered for a language.
    pub fn features_for(&self, lang_id: &str) -> Vec<LanguageFeature> {
        self.features
            .get(lang_id)
            .map_or_else(Vec::new, |set| set.iter().copied().collect())
    }

    /// Return all language IDs that have the given feature registered.
    pub fn languages_with_feature(&self, feature: &LanguageFeature) -> Vec<&str> {
        self.features
            .iter()
            .filter(|(_, set)| set.contains(feature))
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Return the number of features registered for a language.
    pub fn feature_count(&self, lang_id: &str) -> usize {
        self.features.get(lang_id).map_or(0, |set| set.len())
    }

    /// Remove all features for a language.
    pub fn clear_language(&mut self, lang_id: &str) {
        self.features.remove(lang_id);
    }
}

impl Default for LanguageFeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Language / file-extension icons
// ---------------------------------------------------------------------------

/// Return a terminal-friendly icon for the given language ID.
/// Returns a default icon for unknown languages.
pub fn language_icon(language_id: &str) -> &'static str {
    match language_id {
        "rust" => "🦀",
        "python" => "🐍",
        "javascript" => "📜",
        "typescript" => "📘",
        "go" => "🐹",
        "java" => "☕",
        "c" | "cpp" => "⚙",
        "html" => "🌐",
        "css" => "🎨",
        "ruby" => "💎",
        "shell" | "shellscript" => "🐚",
        "markdown" => "📝",
        "json" => "📋",
        _ => "📄",
    }
}

/// Return a short file icon based on file extension.
pub fn file_icon_for_extension(ext: &str) -> &'static str {
    match ext {
        "rs" => "🦀",
        "py" => "🐍",
        "js" | "mjs" | "cjs" => "📜",
        "ts" | "mts" | "cts" => "📘",
        "go" => "🐹",
        "java" => "☕",
        "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" => "⚙",
        "html" | "htm" => "🌐",
        "css" | "scss" | "less" => "🎨",
        "rb" => "💎",
        "sh" | "bash" | "zsh" => "🐚",
        "md" | "markdown" => "📝",
        "json" => "📋",
        _ => "📄",
    }
}

// ---------------------------------------------------------------------------
// Language Bracket Config
// ---------------------------------------------------------------------------

/// Configuration for colorized bracket pairs in a language.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageBracketConfig {
    pub pairs: Vec<(String, String)>,
    pub colorized_pairs: Vec<(String, String)>,
    pub enabled: bool,
}

impl LanguageBracketConfig {
    /// Create a default configuration with `()`, `[]`, `{}` pairs and colorization enabled.
    pub fn new() -> Self {
        let default_pairs = vec![
            ("(".to_string(), ")".to_string()),
            ("[".to_string(), "]".to_string()),
            ("{".to_string(), "}".to_string()),
        ];
        Self {
            colorized_pairs: default_pairs.clone(),
            pairs: default_pairs,
            enabled: true,
        }
    }

    /// Add a bracket pair.
    pub fn add_pair(&mut self, open: &str, close: &str) {
        self.pairs.push((open.to_string(), close.to_string()));
    }

    /// Check whether `ch` is a registered open bracket.
    pub fn is_open_bracket(&self, ch: &str) -> bool {
        self.pairs.iter().any(|(o, _)| o == ch)
    }

    /// Check whether `ch` is a registered close bracket.
    pub fn is_close_bracket(&self, ch: &str) -> bool {
        self.pairs.iter().any(|(_, c)| c == ch)
    }

    /// Return the matching close bracket for the given open bracket, if any.
    pub fn matching_close(&self, open: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(o, _)| o == open)
            .map(|(_, c)| c.as_str())
    }

    /// Return the matching open bracket for the given close bracket, if any.
    pub fn matching_open(&self, close: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(_, c)| c == close)
            .map(|(o, _)| o.as_str())
    }

    /// Return the total number of bracket pairs.
    pub fn pair_count(&self) -> usize {
        self.pairs.len()
    }

    /// Builder-style method to enable or disable colorization.
    pub fn with_colorized(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Default for LanguageBracketConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Language ID normalizer
// ---------------------------------------------------------------------------

/// Normalizes language identifiers by lowercasing and resolving common aliases.
pub struct LanguageIdNormalizer {
    aliases: std::collections::HashMap<String, String>,
}

impl LanguageIdNormalizer {
    /// Create a normalizer pre-loaded with common aliases.
    pub fn new() -> Self {
        let mut aliases = std::collections::HashMap::new();
        for (alias, canonical) in [
            ("js", "javascript"),
            ("jsx", "javascriptreact"),
            ("ts", "typescript"),
            ("tsx", "typescriptreact"),
            ("py", "python"),
            ("rb", "ruby"),
            ("rs", "rust"),
            ("sh", "shellscript"),
            ("bash", "shellscript"),
            ("zsh", "shellscript"),
            ("yml", "yaml"),
            ("md", "markdown"),
            ("cpp", "cpp"),
            ("c++", "cpp"),
            ("cxx", "cpp"),
            ("cc", "cpp"),
            ("objective-c", "objectivec"),
            ("objc", "objectivec"),
            ("dockerfile", "dockerfile"),
            ("docker", "dockerfile"),
            ("make", "makefile"),
        ] {
            aliases.insert(alias.to_string(), canonical.to_string());
        }
        Self { aliases }
    }

    /// Add a custom alias mapping.
    pub fn add_alias(&mut self, alias: &str, canonical: &str) {
        self.aliases
            .insert(alias.to_lowercase(), canonical.to_lowercase());
    }

    /// Normalize a language identifier: lowercase then resolve aliases.
    pub fn normalize(&self, id: &str) -> String {
        let lower = id.to_lowercase();
        self.aliases.get(&lower).cloned().unwrap_or(lower)
    }

    /// Return true if two identifiers refer to the same language.
    pub fn are_equivalent(&self, a: &str, b: &str) -> bool {
        self.normalize(a) == self.normalize(b)
    }

    /// Return the number of registered aliases.
    pub fn alias_count(&self) -> usize {
        self.aliases.len()
    }
}

impl Default for LanguageIdNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LanguageIdNormalizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LanguageIdNormalizer({} aliases)", self.aliases.len())
    }
}

// ---------------------------------------------------------------------------
// Language detector (content-based)
// ---------------------------------------------------------------------------

/// Detects language from file content using shebangs, magic strings, and
/// heuristic patterns beyond simple extension matching.
pub struct LanguageDetector {
    magic_patterns: Vec<(regex::Regex, String)>,
}

impl LanguageDetector {
    /// Create a detector pre-loaded with common content heuristics.
    pub fn new() -> Self {
        let patterns: Vec<(&str, &str)> = vec![
            (r"^#!\s*/usr/bin/env\s+python", "python"),
            (r"^#!\s*/usr/bin/env\s+node", "javascript"),
            (r"^#!\s*/usr/bin/env\s+ruby", "ruby"),
            (r"^#!\s*/usr/bin/env\s+perl", "perl"),
            (r"^#!\s*/usr/bin/env\s+bash", "shellscript"),
            (r"^#!\s*/bin/bash", "shellscript"),
            (r"^#!\s*/bin/sh", "shellscript"),
            (r"^#!\s*/usr/bin/env\s+php", "php"),
            (r"^<\?xml\s", "xml"),
            (r"^<!DOCTYPE\s+html", "html"),
            (r"^<html", "html"),
            (r"^\{", "json"),
            (r"^---\s*$", "yaml"),
        ];
        let magic_patterns = patterns
            .into_iter()
            .filter_map(|(pat, lang)| {
                regex::Regex::new(pat).ok().map(|re| (re, lang.to_string()))
            })
            .collect();
        Self { magic_patterns }
    }

    /// Detect language from the full content of a file.
    ///
    /// Examines the first line for shebangs/magic strings, then applies
    /// heuristic keyword scanning on the body.
    pub fn detect_from_content(&self, content: &str) -> Option<String> {
        let first_line = content.lines().next().unwrap_or("");

        // Try magic patterns on the first line.
        for (re, lang) in &self.magic_patterns {
            if re.is_match(first_line) {
                return Some(lang.clone());
            }
        }

        // Heuristic: scan body for distinctive keywords.
        self.detect_by_keywords(content)
    }

    /// Detect language by scanning for distinctive keyword patterns.
    fn detect_by_keywords(&self, content: &str) -> Option<String> {
        // Count keyword hits for candidate languages.
        let mut scores: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();

        let checks: &[(&[&str], &str)] = &[
            (&["fn ", "let mut ", "impl ", "pub fn ", "use std::"], "rust"),
            (&["def ", "import ", "class ", "if __name__"], "python"),
            (&["func ", "package ", "import (", "go func"], "go"),
            (&["function ", "const ", "=> {", "require("], "javascript"),
            (&["interface ", ": string", ": number", "export "], "typescript"),
            (&["public class ", "System.out", "void main"], "java"),
            (&["#include", "int main(", "printf(", "nullptr"], "cpp"),
        ];

        for (keywords, lang) in checks {
            let hits: u32 = keywords
                .iter()
                .filter(|kw| content.contains(**kw))
                .count() as u32;
            if hits >= 2 {
                *scores.entry(lang).or_default() += hits;
            }
        }

        scores
            .into_iter()
            .max_by_key(|(_, score)| *score)
            .map(|(lang, _)| lang.to_string())
    }

    /// Register an additional magic pattern for first-line detection.
    pub fn add_magic_pattern(&mut self, pattern: &str, language_id: &str) -> Result<(), regex::Error> {
        let re = regex::Regex::new(pattern)?;
        self.magic_patterns.push((re, language_id.to_string()));
        Ok(())
    }

    /// Return the number of registered magic patterns.
    pub fn pattern_count(&self) -> usize {
        self.magic_patterns.len()
    }
}

impl Default for LanguageDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LanguageDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LanguageDetector({} patterns)", self.magic_patterns.len())
    }
}

// ---------------------------------------------------------------------------
// Language comparison
// ---------------------------------------------------------------------------

/// Compares two languages' features and capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageComparison {
    pub lang_a: String,
    pub lang_b: String,
    pub shared_extensions: Vec<String>,
    pub a_only_extensions: Vec<String>,
    pub b_only_extensions: Vec<String>,
    pub same_line_comment: bool,
    pub same_block_comment: bool,
    pub a_has_shebang: bool,
    pub b_has_shebang: bool,
}

impl LanguageComparison {
    /// Compare two languages registered in a [`LanguageService`].
    /// Returns `None` if either language is not registered.
    pub fn compare(svc: &LanguageService, a: &str, b: &str) -> Option<Self> {
        let lang_a = svc.get_language(a)?;
        let lang_b = svc.get_language(b)?;

        let exts_a: std::collections::HashSet<String> =
            lang_a.extensions.iter().map(|e| e.to_lowercase()).collect();
        let exts_b: std::collections::HashSet<String> =
            lang_b.extensions.iter().map(|e| e.to_lowercase()).collect();

        let shared: Vec<String> = exts_a.intersection(&exts_b).cloned().collect();
        let a_only: Vec<String> = exts_a.difference(&exts_b).cloned().collect();
        let b_only: Vec<String> = exts_b.difference(&exts_a).cloned().collect();

        let cfg_a = svc.get_edit_config(a);
        let cfg_b = svc.get_edit_config(b);

        let same_line = match (cfg_a.as_ref(), cfg_b.as_ref()) {
            (Some(ca), Some(cb)) => ca.comments.line_comment == cb.comments.line_comment,
            _ => false,
        };
        let same_block = match (cfg_a.as_ref(), cfg_b.as_ref()) {
            (Some(ca), Some(cb)) => ca.comments.block_comment == cb.comments.block_comment,
            _ => false,
        };

        Some(Self {
            lang_a: a.to_string(),
            lang_b: b.to_string(),
            shared_extensions: shared,
            a_only_extensions: a_only,
            b_only_extensions: b_only,
            same_line_comment: same_line,
            same_block_comment: same_block,
            a_has_shebang: lang_a.first_line.is_some(),
            b_has_shebang: lang_b.first_line.is_some(),
        })
    }

    /// Return a similarity score from 0 to 100.
    pub fn similarity_score(&self) -> u32 {
        let mut score = 0u32;
        if !self.shared_extensions.is_empty() {
            score += 20;
        }
        if self.same_line_comment {
            score += 30;
        }
        if self.same_block_comment {
            score += 30;
        }
        if self.a_has_shebang == self.b_has_shebang {
            score += 20;
        }
        score
    }
}

impl std::fmt::Display for LanguageComparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} vs {} (similarity: {}%)",
            self.lang_a,
            self.lang_b,
            self.similarity_score()
        )
    }
}

// ---------------------------------------------------------------------------
// Language filter (builder pattern)
// ---------------------------------------------------------------------------

/// Builder for querying languages by features.
pub struct LanguageFilter<'a> {
    svc: &'a LanguageService,
    require_block_comments: Option<bool>,
    require_shebangs: Option<bool>,
    extension_pattern: Option<String>,
    min_extensions: Option<usize>,
    require_line_comments: Option<bool>,
}

impl<'a> LanguageFilter<'a> {
    /// Create a new filter targeting the given service.
    pub fn new(svc: &'a LanguageService) -> Self {
        Self {
            svc,
            require_block_comments: None,
            require_shebangs: None,
            extension_pattern: None,
            min_extensions: None,
            require_line_comments: None,
        }
    }

    /// Only include languages that have (or lack) block comments.
    pub fn has_block_comments(mut self, yes: bool) -> Self {
        self.require_block_comments = Some(yes);
        self
    }

    /// Only include languages that have (or lack) line comments.
    pub fn has_line_comments(mut self, yes: bool) -> Self {
        self.require_line_comments = Some(yes);
        self
    }

    /// Only include languages that have (or lack) shebang/first-line patterns.
    pub fn has_shebangs(mut self, yes: bool) -> Self {
        self.require_shebangs = Some(yes);
        self
    }

    /// Only include languages with an extension matching the given substring.
    pub fn extension_contains(mut self, pattern: &str) -> Self {
        self.extension_pattern = Some(pattern.to_lowercase());
        self
    }

    /// Only include languages with at least `n` registered extensions.
    pub fn min_extensions(mut self, n: usize) -> Self {
        self.min_extensions = Some(n);
        self
    }

    /// Execute the filter and return matching language IDs.
    pub fn execute(&self) -> Vec<String> {
        self.svc
            .get_registered_language_ids()
            .into_iter()
            .filter(|id| self.matches(id))
            .map(|id| id.to_string())
            .collect()
    }

    fn matches(&self, id: &str) -> bool {
        let lang = match self.svc.get_language(id) {
            Some(l) => l,
            None => return false,
        };

        if let Some(need_shebang) = self.require_shebangs {
            if lang.first_line.is_some() != need_shebang {
                return false;
            }
        }

        if let Some(min) = self.min_extensions {
            if lang.extensions.len() < min {
                return false;
            }
        }

        if let Some(ref pat) = self.extension_pattern {
            if !lang.extensions.iter().any(|e| e.to_lowercase().contains(pat)) {
                return false;
            }
        }

        if let Some(need_block) = self.require_block_comments {
            let has = self
                .svc
                .get_edit_config(id)
                .map_or(false, |cfg| cfg.comments.block_comment.is_some());
            if has != need_block {
                return false;
            }
        }

        if let Some(need_line) = self.require_line_comments {
            let has = self
                .svc
                .get_edit_config(id)
                .map_or(false, |cfg| cfg.comments.line_comment.is_some());
            if has != need_line {
                return false;
            }
        }

        true
    }
}

impl std::fmt::Display for LanguageFilter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if let Some(v) = self.require_block_comments {
            parts.push(format!("block_comments={v}"));
        }
        if let Some(v) = self.require_line_comments {
            parts.push(format!("line_comments={v}"));
        }
        if let Some(v) = self.require_shebangs {
            parts.push(format!("shebangs={v}"));
        }
        if let Some(ref p) = self.extension_pattern {
            parts.push(format!("ext~{p}"));
        }
        if let Some(n) = self.min_extensions {
            parts.push(format!("min_ext={n}"));
        }
        if parts.is_empty() {
            write!(f, "LanguageFilter(no constraints)")
        } else {
            write!(f, "LanguageFilter({})", parts.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// Display impls for existing types
// ---------------------------------------------------------------------------

impl std::fmt::Display for LanguageStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} languages, {} extensions, {} with shebangs, {} with block comments",
            self.total_languages,
            self.total_extensions,
            self.languages_with_shebangs,
            self.languages_with_block_comments,
        )
    }
}

impl std::fmt::Display for LanguageSimilarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (shared: {})",
            self.language_id,
            self.shared_extensions.join(", ")
        )
    }
}

impl std::fmt::Display for LanguageMatchScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (score: {})", self.language_id, self.score())
    }
}

impl std::fmt::Display for ShebangInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.args.is_empty() {
            write!(f, "{}", self.interpreter_name)
        } else {
            write!(f, "{} {}", self.interpreter_name, self.args.join(" "))
        }
    }
}

impl std::fmt::Display for LanguageBracketConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} bracket pairs (colorized: {})",
            self.pairs.len(),
            self.enabled,
        )
    }
}

impl From<&str> for LanguageBracketConfig {
    /// Create a bracket config from a comma-separated list of open/close pairs
    /// like `"(),[],{}"`.
    fn from(s: &str) -> Self {
        let mut cfg = Self {
            pairs: Vec::new(),
            colorized_pairs: Vec::new(),
            enabled: true,
        };
        for pair in s.split(',') {
            let chars: Vec<char> = pair.chars().collect();
            if chars.len() == 2 {
                let open = chars[0].to_string();
                let close = chars[1].to_string();
                cfg.colorized_pairs.push((open.clone(), close.clone()));
                cfg.pairs.push((open, close));
            }
        }
        cfg
    }
}

// ---------------------------------------------------------------------------
// Language auto-detection by content patterns
// ---------------------------------------------------------------------------

/// Detects language by file content patterns such as shebangs, magic comments,
/// and XML declarations. Unlike [`LanguageDetector`] which uses built-in
/// heuristics, this type focuses on explicit first-line markers and allows
/// full customization of patterns.
pub struct LanguageAutoDetector {
    patterns: Vec<(regex::Regex, String)>,
}

impl LanguageAutoDetector {
    /// Create a detector pre-loaded with common content patterns.
    pub fn new() -> Self {
        let defaults: &[(&str, &str)] = &[
            (r"^#!\s*/usr/bin/env\s+(\S+)", ""),   // generic shebang (env form)
            (r"^#!\s*/bin/(\S+)", ""),               // generic shebang (direct form)
            (r"^<\?xml\b", "xml"),
            (r"^<!DOCTYPE\s+html", "html"),
            (r"^#\s*-\*-\s*mode:\s*(\S+)", ""),      // Emacs mode line
            (r"^//\s*-\*-\s*mode:\s*(\S+)", ""),      // C-style mode line
            (r"^<\?php", "php"),
        ];
        let mut patterns = Vec::new();
        for &(pat, lang) in defaults {
            if let Ok(re) = regex::Regex::new(pat) {
                patterns.push((re, lang.to_string()));
            }
        }
        Self { patterns }
    }

    /// Detect language from the full file content.
    ///
    /// Checks the first line against all registered patterns. For shebang
    /// patterns the interpreter name is mapped to a canonical language id.
    pub fn detect_by_content(&self, content: &str) -> Option<String> {
        let first_line = content.lines().next().unwrap_or("");
        self.detect_by_first_line(first_line)
    }

    /// Detect language from just the first line of a file.
    pub fn detect_by_first_line(&self, line: &str) -> Option<String> {
        let trimmed = line.trim();
        for (re, lang) in &self.patterns {
            if let Some(caps) = re.captures(trimmed) {
                if !lang.is_empty() {
                    return Some(lang.clone());
                }
                // Extract the captured interpreter/mode name.
                if let Some(m) = caps.get(1) {
                    return Some(Self::normalize_interpreter(m.as_str()));
                }
            }
        }
        None
    }

    /// Register a custom pattern. The pattern is tested against the first line
    /// of the file; if it matches, `language_id` is returned.
    pub fn add_pattern(&mut self, pattern: &str, language_id: &str) -> Result<(), regex::Error> {
        let re = regex::Regex::new(pattern)?;
        self.patterns.push((re, language_id.to_string()));
        Ok(())
    }

    /// Return the number of registered patterns.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Map common interpreter names to canonical language identifiers.
    fn normalize_interpreter(name: &str) -> String {
        match name {
            "python" | "python3" | "python2" => "python".to_string(),
            "node" | "nodejs" => "javascript".to_string(),
            "ruby" | "irb" => "ruby".to_string(),
            "perl" | "perl5" | "perl6" => "perl".to_string(),
            "bash" | "sh" | "zsh" | "fish" | "dash" => "shellscript".to_string(),
            "php" => "php".to_string(),
            "lua" | "luajit" => "lua".to_string(),
            other => other.to_string(),
        }
    }
}

impl Default for LanguageAutoDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LanguageAutoDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LanguageAutoDetector({} patterns)", self.patterns.len())
    }
}

// ---------------------------------------------------------------------------
// Language configuration resolver
// ---------------------------------------------------------------------------

/// Resolved editing configuration for a language, with easy access to comment
/// style and auto-closing pairs.
#[derive(Debug, Clone)]
pub struct LanguageResolvedEditConfig {
    /// Line comment prefix, if any (e.g. `"//"`).
    pub line_comment: Option<String>,
    /// Block comment opening delimiter, if any (e.g. `"/*"`).
    pub block_comment_start: Option<String>,
    /// Block comment closing delimiter, if any (e.g. `"*/"`).
    pub block_comment_end: Option<String>,
    /// Auto-closing pairs as `(open, close)` tuples.
    pub auto_closing_pairs: Vec<(String, String)>,
}

/// Resolves comment style and bracket configuration for a language by
/// consulting a [`LanguageService`].
pub struct LanguageConfigurationResolver<'a> {
    svc: &'a LanguageService,
}

impl<'a> LanguageConfigurationResolver<'a> {
    /// Create a resolver backed by the given service.
    pub fn new(svc: &'a LanguageService) -> Self {
        Self { svc }
    }

    /// Resolve the editing configuration for `language_id`.
    ///
    /// Returns a simplified [`LanguageResolvedEditConfig`] with comment and
    /// auto-closing pair information. If the language has no registered edit
    /// config, sensible defaults are returned.
    pub fn resolve(&self, language_id: &str) -> LanguageResolvedEditConfig {
        match self.svc.get_edit_config(language_id) {
            Some(cfg) => {
                let (bcs, bce) = match &cfg.comments.block_comment {
                    Some((s, e)) => (Some(s.clone()), Some(e.clone())),
                    None => (None, None),
                };
                LanguageResolvedEditConfig {
                    line_comment: cfg.comments.line_comment.clone(),
                    block_comment_start: bcs,
                    block_comment_end: bce,
                    auto_closing_pairs: cfg
                        .auto_closing_pairs
                        .iter()
                        .map(|p| (p.open.clone(), p.close.clone()))
                        .collect(),
                }
            }
            None => LanguageResolvedEditConfig {
                line_comment: None,
                block_comment_start: None,
                block_comment_end: None,
                auto_closing_pairs: Vec::new(),
            },
        }
    }

    /// Check whether a language supports line comments.
    pub fn has_line_comment(&self, language_id: &str) -> bool {
        self.resolve(language_id).line_comment.is_some()
    }

    /// Check whether a language supports block comments.
    pub fn has_block_comment(&self, language_id: &str) -> bool {
        self.resolve(language_id).block_comment_start.is_some()
    }
}

impl std::fmt::Display for LanguageConfigurationResolver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LanguageConfigurationResolver")
    }
}

// ---------------------------------------------------------------------------
// Embedded language scope tracker
// ---------------------------------------------------------------------------

/// Tracks a stack of embedded languages, e.g. JavaScript inside HTML `<script>`
/// tags, or CSS inside `<style>` tags.
///
/// The bottom of the stack is the host language; subsequent entries represent
/// nested embedded regions.
#[derive(Debug, Clone)]
pub struct LanguageScope {
    stack: Vec<String>,
}

impl LanguageScope {
    /// Create a scope tracker rooted at `host_language`.
    pub fn new(host_language: &str) -> Self {
        Self {
            stack: vec![host_language.to_string()],
        }
    }

    /// Push an embedded language onto the scope stack.
    pub fn push_scope(&mut self, lang_id: &str) {
        self.stack.push(lang_id.to_string());
    }

    /// Pop the most recently pushed scope, returning its language id.
    ///
    /// The host language (bottom of the stack) is never popped; attempting to
    /// pop when only the host remains returns `None`.
    pub fn pop_scope(&mut self) -> Option<String> {
        if self.stack.len() > 1 {
            self.stack.pop()
        } else {
            None
        }
    }

    /// Return the language id of the current (innermost) scope.
    pub fn current_language(&self) -> &str {
        self.stack.last().map(|s| s.as_str()).unwrap_or("plaintext")
    }

    /// Return the nesting depth. A depth of 1 means only the host language is
    /// active (no embedded regions).
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Return the host (root) language.
    pub fn host_language(&self) -> &str {
        self.stack.first().map(|s| s.as_str()).unwrap_or("plaintext")
    }

    /// Return `true` if currently inside an embedded region.
    pub fn is_embedded(&self) -> bool {
        self.stack.len() > 1
    }
}

impl std::fmt::Display for LanguageScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LanguageScope({})", self.stack.join(" > "))
    }
}

// ---------------------------------------------------------------------------
// Language alias mapper
// ---------------------------------------------------------------------------

/// Bidirectional mapper between language aliases and canonical language
/// identifiers.
///
/// Unlike [`LanguageIdNormalizer`], this mapper supports reverse lookups:
/// given a canonical id, find all known aliases.
pub struct LanguageAliasMapper {
    /// alias (lowercased) → canonical id
    to_canonical: std::collections::HashMap<String, String>,
    /// canonical id → set of aliases
    from_canonical: std::collections::HashMap<String, Vec<String>>,
}

impl LanguageAliasMapper {
    /// Create an empty mapper.
    pub fn new() -> Self {
        Self {
            to_canonical: std::collections::HashMap::new(),
            from_canonical: std::collections::HashMap::new(),
        }
    }

    /// Register `alias` as an alternate name for `canonical`.
    pub fn add_alias(&mut self, alias: &str, canonical: &str) {
        let alias_lower = alias.to_lowercase();
        let canon_lower = canonical.to_lowercase();
        self.to_canonical
            .insert(alias_lower.clone(), canon_lower.clone());
        self.from_canonical
            .entry(canon_lower)
            .or_default()
            .push(alias_lower);
    }

    /// Resolve a name to its canonical language id.
    ///
    /// If the name is not a known alias it is returned as-is (lowercased).
    pub fn resolve<'a>(&'a self, name: &'a str) -> &'a str {
        let lower = name.to_lowercase();
        // Need to check the map; if found return the stored canonical value,
        // otherwise return name unchanged.
        match self.to_canonical.get(&lower) {
            Some(canonical) => canonical.as_str(),
            None => name,
        }
    }

    /// Return all aliases registered for `canonical`.
    pub fn aliases_for(&self, canonical: &str) -> Vec<String> {
        let canon_lower = canonical.to_lowercase();
        self.from_canonical
            .get(&canon_lower)
            .cloned()
            .unwrap_or_default()
    }

    /// Return the total number of alias mappings.
    pub fn alias_count(&self) -> usize {
        self.to_canonical.len()
    }

    /// Return `true` if two names resolve to the same canonical id.
    pub fn are_equivalent(&self, a: &str, b: &str) -> bool {
        let ra = self.to_canonical.get(&a.to_lowercase()).map(|s| s.as_str()).unwrap_or(a);
        let rb = self.to_canonical.get(&b.to_lowercase()).map(|s| s.as_str()).unwrap_or(b);
        ra.eq_ignore_ascii_case(rb)
    }
}

impl Default for LanguageAliasMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LanguageAliasMapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LanguageAliasMapper({} aliases, {} canonical ids)",
            self.to_canonical.len(),
            self.from_canonical.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ─── LangC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for language detect.
#[derive(Debug)]
pub struct LangCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> LangCLruCache<V> {
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

impl<V: Clone + fmt::Display> fmt::Display for LangCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LangCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── LangB Builder & Validator ─────────────────────────────

/// Builder for constructing language configurations.
#[derive(Debug, Clone)]
pub struct LangBBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl LangBBuilder {
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

    pub fn build(self) -> Result<LangBCfg, LangBBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(LangBBuildErr { errors }); }
        Ok(LangBCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated language configuration.
#[derive(Debug, Clone)]
pub struct LangBCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl LangBCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &LangBCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for LangBCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LangBCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct LangBBuildErr { pub errors: Vec<String> }

impl fmt::Display for LangBBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LangBBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for LangBBuildErr {}


/// Language configuration manager.
#[derive(Debug, Clone)]
pub struct LanguagesConfig {
    entries: Vec<LanguagesEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single language entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguagesEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl LanguagesEntry {
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

impl LanguagesConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: LanguagesEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&LanguagesEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut LanguagesEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&LanguagesEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&LanguagesEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&LanguagesEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<LanguagesEntry> {
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
// xa_ extended helpers for languages
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaLanguagesRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaLanguagesRingBuf {
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
pub struct XaLanguagesCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaLanguagesCounter {
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

impl Default for XaLanguagesCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 108
// ---------------------------------------------------------------------------

/// Generic object pool `Xc108Pool<T>`.
pub struct Xc108Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc108Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc108PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc108Pool<T> {
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
    pub fn stats(&self) -> Xc108PoolStats {
        Xc108PoolStats {
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

impl<T> Default for Xc108Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc108Scheduler`.
pub struct Xc108Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc108Scheduler {
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

impl Default for Xc108Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_108 hash for the given byte slice.
pub fn xc_108_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_108 convention.
pub fn xc_108_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_75 deepening: state machine + event bus ---

/// States for the Xd75 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd75State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd75State {
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
pub struct Xd75Transition {
    pub from: Xd75State,
    pub to: Xd75State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd75StateMachine {
    current: Xd75State,
    history: Vec<Xd75Transition>,
    step_counter: usize,
}

impl Xd75StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd75State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd75State {
        self.current
    }

    pub fn history(&self) -> &[Xd75Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd75State) -> Result<Xd75State, String> {
        let allowed = match (self.current, target) {
            (Xd75State::Idle, Xd75State::Running) => true,
            (Xd75State::Running, Xd75State::Paused) => true,
            (Xd75State::Running, Xd75State::Done) => true,
            (Xd75State::Paused, Xd75State::Running) => true,
            (Xd75State::Paused, Xd75State::Done) => true,
            (Xd75State::Done, Xd75State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_75: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd75Transition {
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
            "Xd75SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd75State> {
        let prefix = "Xd75SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd75State::Idle),
            "Running" => Some(Xd75State::Running),
            "Paused" => Some(Xd75State::Paused),
            "Done" => Some(Xd75State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd75State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd75 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd75Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd75Event {
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

type Xd75HandlerFn = Box<dyn Fn(&Xd75Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd75EventBus {
    handlers: Vec<(usize, Option<String>, Xd75HandlerFn)>,
    next_id: usize,
    published: Vec<Xd75Event>,
}

impl Xd75EventBus {
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
        F: Fn(&Xd75Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd75Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd75Event) {
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

    pub fn published_events(&self) -> &[Xd75Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #93
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf93Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf93TrieNode {
    children: std::collections::HashMap<char, Xf93TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf93Trie {
    root: Xf93TrieNode,
    count: usize,
}

impl Xf93Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf93TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf93TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf93TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf93BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf93BloomFilter {
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

    fn make_registry() -> LanguageService {
        let mut reg = LanguageService::new();
        register_default_languages(&mut reg);
        reg
    }

    // -- Extension lookup ---------------------------------------------------

    #[test]
    fn extension_lookup_rust() {
        let reg = make_registry();
        assert_eq!(reg.get_language_id_by_extension(".rs"), Some("rust"));
    }

    #[test]
    fn extension_lookup_typescript() {
        let reg = make_registry();
        assert_eq!(reg.get_language_id_by_extension(".ts"), Some("typescript"));
    }

    #[test]
    fn extension_lookup_case_insensitive() {
        let reg = make_registry();
        assert_eq!(reg.get_language_id_by_extension(".RS"), Some("rust"));
        assert_eq!(reg.get_language_id_by_extension(".Py"), Some("python"));
    }

    #[test]
    fn extension_lookup_unknown() {
        let reg = make_registry();
        assert_eq!(reg.get_language_id_by_extension(".xyz"), None);
    }

    // -- Filename lookup ----------------------------------------------------

    #[test]
    fn filename_lookup_dockerfile() {
        let reg = make_registry();
        assert_eq!(reg.get_language_id_by_filename("Dockerfile"), Some("dockerfile"));
    }

    #[test]
    fn filename_lookup_cargo_toml() {
        let reg = make_registry();
        assert_eq!(reg.get_language_id_by_filename("Cargo.toml"), Some("toml"));
    }

    #[test]
    fn filename_lookup_bashrc() {
        let reg = make_registry();
        assert_eq!(reg.get_language_id_by_filename(".bashrc"), Some("shellscript"));
    }

    #[test]
    fn filename_lookup_unknown() {
        let reg = make_registry();
        assert_eq!(reg.get_language_id_by_filename("random_file"), None);
    }

    // -- MIME type lookup ---------------------------------------------------

    #[test]
    fn mime_lookup() {
        let reg = make_registry();
        assert_eq!(reg.get_language_id_by_mime("text/html"), Some("html"));
        assert_eq!(reg.get_language_id_by_mime("text/css"), Some("css"));
        assert_eq!(reg.get_language_id_by_mime("application/xml"), Some("xml"));
    }

    // -- First-line detection -----------------------------------------------

    #[test]
    fn first_line_python_shebang() {
        let reg = make_registry();
        assert_eq!(
            reg.get_language_id_by_first_line("#!/usr/bin/env python3"),
            Some("python"),
        );
    }

    #[test]
    fn first_line_bash_shebang() {
        let reg = make_registry();
        assert_eq!(
            reg.get_language_id_by_first_line("#!/bin/bash"),
            Some("shellscript"),
        );
    }

    #[test]
    fn first_line_node_shebang() {
        let reg = make_registry();
        assert_eq!(
            reg.get_language_id_by_first_line("#!/usr/bin/env node"),
            Some("javascript"),
        );
    }

    #[test]
    fn first_line_xml_declaration() {
        let reg = make_registry();
        assert_eq!(
            reg.get_language_id_by_first_line("<?xml version=\"1.0\"?>"),
            Some("xml"),
        );
    }

    #[test]
    fn first_line_no_match() {
        let reg = make_registry();
        assert_eq!(reg.get_language_id_by_first_line("hello world"), None);
    }

    // -- guess_language_id --------------------------------------------------

    #[test]
    fn guess_by_filename_takes_priority() {
        let reg = make_registry();
        assert_eq!(reg.guess_language_id("Dockerfile", None), Some("dockerfile"));
    }

    #[test]
    fn guess_by_extension() {
        let reg = make_registry();
        assert_eq!(reg.guess_language_id("main.rs", None), Some("rust"));
        assert_eq!(reg.guess_language_id("src/app.ts", None), Some("typescript"));
    }

    #[test]
    fn guess_by_first_line_fallback() {
        let reg = make_registry();
        assert_eq!(
            reg.guess_language_id("my_script", Some("#!/usr/bin/env python3")),
            Some("python"),
        );
    }

    #[test]
    fn guess_no_match() {
        let reg = make_registry();
        assert_eq!(reg.guess_language_id("unknown", None), None);
    }

    #[test]
    fn guess_full_path_extracts_basename() {
        let reg = make_registry();
        assert_eq!(
            reg.guess_language_id("/home/user/project/Makefile.rs", None),
            Some("rust"),
        );
    }

    // -- get_language / get_registered_language_ids --------------------------

    #[test]
    fn get_language_by_id() {
        let reg = make_registry();
        let lang = reg.get_language("rust").unwrap();
        assert_eq!(lang.name, "Rust");
        assert!(lang.extensions.contains(&".rs".to_string()));
    }

    #[test]
    fn get_language_unknown() {
        let reg = make_registry();
        assert!(reg.get_language("brainfuck").is_none());
    }

    #[test]
    fn registered_language_ids_contains_defaults() {
        let reg = make_registry();
        let ids = reg.get_registered_language_ids();
        assert!(ids.contains(&"rust"));
        assert!(ids.contains(&"python"));
        assert!(ids.contains(&"dockerfile"));
        assert!(ids.len() >= 30);
    }

    // -- Custom registration ------------------------------------------------

    #[test]
    fn custom_language_registration() {
        let mut reg = LanguageService::new();
        reg.register(LanguageDefinition {
            id: "brainfuck".into(),
            name: "Brainfuck".into(),
            extensions: vec![".bf".into()],
            filenames: vec![],
            aliases: vec!["Brainfuck".into()],
            mime_types: vec![],
            first_line: None,
        });
        assert_eq!(reg.get_language_id_by_extension(".bf"), Some("brainfuck"));
        assert_eq!(reg.guess_language_id("hello.bf", None), Some("brainfuck"));
    }

    // -- Default trait ------------------------------------------------------

    #[test]
    fn default_creates_empty_registry() {
        let reg = LanguageService::default();
        assert!(reg.get_registered_language_ids().is_empty());
    }

    // -- New language registrations -----------------------------------------

    #[test]
    fn ruby_registered() {
        let reg = make_registry();
        assert_eq!(reg.guess_language_id("app.rb", None), Some("ruby"));
    }

    #[test]
    fn php_registered() {
        let reg = make_registry();
        assert_eq!(reg.guess_language_id("index.php", None), Some("php"));
    }

    #[test]
    fn makefile_registered() {
        let reg = make_registry();
        assert_eq!(reg.guess_language_id("Makefile", None), Some("makefile"));
    }

    #[test]
    fn perl_shebang() {
        let reg = make_registry();
        assert_eq!(
            reg.get_language_id_by_first_line("#!/usr/bin/perl"),
            Some("perl"),
        );
    }

    #[test]
    fn ruby_shebang() {
        let reg = make_registry();
        assert_eq!(
            reg.get_language_id_by_first_line("#!/usr/bin/env ruby"),
            Some("ruby"),
        );
    }

    // -- LanguageEditConfig -------------------------------------------------

    #[test]
    fn default_edit_config_has_common_brackets() {
        let cfg = LanguageEditConfig::default();
        assert!(cfg.brackets.iter().any(|b| b.open == "(" && b.close == ")"));
        assert!(cfg.brackets.iter().any(|b| b.open == "{" && b.close == "}"));
    }

    #[test]
    fn edit_config_for_rust() {
        let svc = make_registry();
        let cfg = svc.get_edit_config("rust").unwrap();
        assert_eq!(cfg.comments.line_comment.as_deref(), Some("//"));
        let (open, close) = cfg.comments.block_comment.as_ref().unwrap();
        assert_eq!(open, "/*");
        assert_eq!(close, "*/");
    }

    #[test]
    fn edit_config_for_python() {
        let svc = make_registry();
        let cfg = svc.get_edit_config("python").unwrap();
        assert_eq!(cfg.comments.line_comment.as_deref(), Some("#"));
        assert!(cfg.comments.block_comment.is_none());
    }

    #[test]
    fn edit_config_for_html() {
        let svc = make_registry();
        let cfg = svc.get_edit_config("html").unwrap();
        assert!(cfg.comments.line_comment.is_none());
        let (open, close) = cfg.comments.block_comment.as_ref().unwrap();
        assert_eq!(open, "<!--");
        assert_eq!(close, "-->");
    }

    #[test]
    fn edit_config_unknown_language() {
        let svc = make_registry();
        assert!(svc.get_edit_config("brainfuck").is_none());
    }

    // -- Comment toggling ---------------------------------------------------

    #[test]
    fn toggle_line_comment_adds() {
        let result = toggle_line_comment("//", "hello world", false);
        assert_eq!(result, "// hello world");
    }

    #[test]
    fn toggle_line_comment_removes() {
        let result = toggle_line_comment("//", "// hello world", true);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn toggle_line_comment_removes_no_space() {
        let result = toggle_line_comment("//", "//hello world", true);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn toggle_block_comment_adds() {
        let result = toggle_block_comment("/*", "*/", "hello world", false);
        assert_eq!(result, "/* hello world */");
    }

    #[test]
    fn toggle_block_comment_removes() {
        let result = toggle_block_comment("/*", "*/", "/* hello world */", true);
        assert_eq!(result, "hello world");
    }

    // -- Auto-closing -------------------------------------------------------

    #[test]
    fn should_auto_close_bracket() {
        let cfg = LanguageEditConfig::default();
        assert!(should_auto_close(&cfg, "(", "normal"));
    }

    #[test]
    fn should_auto_close_respects_not_in() {
        let mut cfg = LanguageEditConfig::default();
        cfg.auto_closing_pairs = vec![AutoClosingPair {
            open: "'".into(),
            close: "'".into(),
            not_in: vec!["string".into(), "comment".into()],
        }];
        assert!(!should_auto_close(&cfg, "'", "string"));
        assert!(!should_auto_close(&cfg, "'", "comment"));
        assert!(should_auto_close(&cfg, "'", "normal"));
    }

    #[test]
    fn get_matching_bracket_found() {
        let cfg = LanguageEditConfig::default();
        assert_eq!(get_matching_bracket(&cfg, "("), Some(")".to_string()));
        assert_eq!(get_matching_bracket(&cfg, "{"), Some("}".to_string()));
        assert_eq!(get_matching_bracket(&cfg, "["), Some("]".to_string()));
    }

    #[test]
    fn get_matching_bracket_not_found() {
        let cfg = LanguageEditConfig::default();
        assert_eq!(get_matching_bracket(&cfg, "x"), None);
    }

    // -- Word pattern -------------------------------------------------------

    #[test]
    fn default_word_pattern_matches() {
        let svc = make_registry();
        let re = get_word_pattern(&svc, "rust");
        assert!(re.is_match("hello"));
        assert!(re.is_match("_foo"));
        assert!(!re.is_match("123"));
    }

    #[test]
    fn javascript_word_pattern_includes_dollar() {
        let svc = make_registry();
        let re = get_word_pattern(&svc, "javascript");
        assert!(re.is_match("$element"));
    }

    #[test]
    fn css_word_pattern_includes_dash() {
        let svc = make_registry();
        let re = get_word_pattern(&svc, "css");
        assert!(re.is_match("font-size"));
    }

    // -- parse_language_configuration (JSON) --------------------------------

    #[test]
    fn parse_language_configuration_json() {
        let json = r#"{
            "comments": {
                "lineComment": "//",
                "blockComment": ["/*", "*/"]
            },
            "brackets": [
                ["(", ")"],
                ["{", "}"]
            ],
            "autoClosingPairs": [
                { "open": "{", "close": "}" },
                { "open": "'", "close": "'", "notIn": ["string"] }
            ],
            "folding": {
                "markers": {
                    "start": "^\\s*#region\\b",
                    "end": "^\\s*#endregion\\b"
                }
            }
        }"#;
        let cfg = parse_language_configuration(json).unwrap();
        assert_eq!(cfg.comments.line_comment.as_deref(), Some("//"));
        let (open, close) = cfg.comments.block_comment.as_ref().unwrap();
        assert_eq!(open, "/*");
        assert_eq!(close, "*/");
        assert_eq!(cfg.brackets.len(), 2);
        assert_eq!(cfg.auto_closing_pairs.len(), 2);
        assert_eq!(cfg.auto_closing_pairs[1].not_in, vec!["string".to_string()]);
        assert!(cfg.folding_markers.is_some());
        let fm = cfg.folding_markers.as_ref().unwrap();
        assert!(fm.start.is_match("  #region foo"));
    }

    #[test]
    fn parse_language_configuration_minimal() {
        let json = r#"{}"#;
        let cfg = parse_language_configuration(json).unwrap();
        assert!(cfg.comments.line_comment.is_none());
        assert!(cfg.brackets.is_empty());
    }

    // -- Folding markers ----------------------------------------------------

    #[test]
    fn edit_config_folding_markers_rust() {
        let svc = make_registry();
        let cfg = svc.get_edit_config("rust").unwrap();
        let fm = cfg.folding_markers.as_ref().unwrap();
        assert!(fm.start.is_match("  // #region test"));
        assert!(fm.end.is_match("  // #endregion"));
    }

    // -- Indentation rules --------------------------------------------------

    #[test]
    fn indentation_rules_present_for_rust() {
        let svc = make_registry();
        let cfg = svc.get_edit_config("rust").unwrap();
        let rules = cfg.indentation_rules.as_ref().unwrap();
        assert!(rules.increase_indent_pattern.is_match("{"));
    }

    // -- Surrounding pairs --------------------------------------------------

    #[test]
    fn surrounding_pairs_present() {
        let cfg = LanguageEditConfig::default();
        assert!(cfg.surrounding_pairs.iter().any(|b| b.open == "(" && b.close == ")"));
        assert!(cfg.surrounding_pairs.iter().any(|b| b.open == "\"" && b.close == "\""));
    }

    // -- OnEnterRule --------------------------------------------------------

    #[test]
    fn on_enter_rules_for_c_like() {
        let svc = make_registry();
        let cfg = svc.get_edit_config("javascript").unwrap();
        assert!(!cfg.on_enter_rules.is_empty());
        // The first rule should match opening a block comment
        let rule = &cfg.on_enter_rules[0];
        assert!(rule.before_text.is_match("/** some doc"));
    }

    // -- compute_language_stats ---------------------------------------------

    #[test]
    fn stats_total_languages_matches_registered() {
        let svc = make_registry();
        let stats = compute_language_stats(&svc);
        assert_eq!(stats.total_languages, svc.get_registered_language_ids().len());
        assert!(stats.total_languages >= 30);
    }

    #[test]
    fn stats_total_extensions_positive() {
        let svc = make_registry();
        let stats = compute_language_stats(&svc);
        assert!(stats.total_extensions > stats.total_languages);
    }

    #[test]
    fn stats_shebangs_positive() {
        let svc = make_registry();
        let stats = compute_language_stats(&svc);
        // At least python, shellscript, ruby, perl have shebangs
        assert!(stats.languages_with_shebangs >= 4);
    }

    #[test]
    fn stats_block_comments_positive() {
        let svc = make_registry();
        let stats = compute_language_stats(&svc);
        // Rust, JavaScript, C, etc. have block comments
        assert!(stats.languages_with_block_comments >= 3);
    }

    #[test]
    fn stats_empty_service() {
        let svc = LanguageService::new();
        let stats = compute_language_stats(&svc);
        assert_eq!(stats, LanguageStats {
            total_languages: 0,
            total_extensions: 0,
            languages_with_shebangs: 0,
            languages_with_block_comments: 0,
        });
    }

    // -- find_similar_languages ---------------------------------------------

    #[test]
    fn similar_languages_unknown_returns_empty() {
        let svc = make_registry();
        let similar = find_similar_languages(&svc, "nonexistent");
        assert!(similar.is_empty());
    }

    #[test]
    fn similar_languages_custom_overlap() {
        let mut svc = LanguageService::new();
        svc.register(LanguageDefinition {
            id: "lang_a".into(),
            name: "Lang A".into(),
            extensions: vec![".x".into(), ".y".into()],
            filenames: vec![],
            aliases: vec![],
            mime_types: vec![],
            first_line: None,
        });
        svc.register(LanguageDefinition {
            id: "lang_b".into(),
            name: "Lang B".into(),
            extensions: vec![".x".into(), ".z".into()],
            filenames: vec![],
            aliases: vec![],
            mime_types: vec![],
            first_line: None,
        });
        svc.register(LanguageDefinition {
            id: "lang_c".into(),
            name: "Lang C".into(),
            extensions: vec![".z".into()],
            filenames: vec![],
            aliases: vec![],
            mime_types: vec![],
            first_line: None,
        });

        let similar = find_similar_languages(&svc, "lang_a");
        assert_eq!(similar.len(), 1);
        assert_eq!(similar[0].language_id, "lang_b");
        assert_eq!(similar[0].shared_extensions, vec![".x".to_string()]);
    }

    #[test]
    fn similar_languages_does_not_include_self() {
        let svc = make_registry();
        let similar = find_similar_languages(&svc, "rust");
        assert!(similar.iter().all(|s| s.language_id != "rust"));
    }

    // -- bulk_detect --------------------------------------------------------

    #[test]
    fn bulk_detect_multiple_files() {
        let svc = make_registry();
        let results = bulk_detect(&svc, &["main.rs", "index.html", "unknown.xyz"]);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], ("main.rs".to_string(), Some("rust")));
        assert_eq!(results[1], ("index.html".to_string(), Some("html")));
        assert_eq!(results[2], ("unknown.xyz".to_string(), None));
    }

    #[test]
    fn bulk_detect_empty_input() {
        let svc = make_registry();
        let results = bulk_detect(&svc, &[]);
        assert!(results.is_empty());
    }

    #[test]
    fn bulk_detect_filenames_and_extensions() {
        let svc = make_registry();
        let results = bulk_detect(&svc, &["Dockerfile", "app.py", "style.css", "Makefile"]);
        assert_eq!(results[0].1, Some("dockerfile"));
        assert_eq!(results[1].1, Some("python"));
        assert_eq!(results[2].1, Some("css"));
        assert_eq!(results[3].1, Some("makefile"));
    }

    // -- score_languages ----------------------------------------------------

    #[test]
    fn score_languages_rust_file() {
        let svc = make_registry();
        let scores = score_languages(&svc, "main.rs", None);
        assert!(!scores.is_empty());
        assert_eq!(scores[0].language_id, "rust");
        assert!(scores[0].extension_match);
        assert!(scores[0].score() >= 5);
    }

    #[test]
    fn score_languages_with_first_line() {
        let svc = make_registry();
        let scores = score_languages(&svc, "script", Some("#!/usr/bin/env python3"));
        assert!(scores.iter().any(|s| s.language_id == "python" && s.first_line_match));
    }

    #[test]
    fn score_languages_no_match() {
        let svc = make_registry();
        let scores = score_languages(&svc, "unknown.xyz", None);
        assert!(scores.is_empty());
    }

    // -- find_extension_conflicts -------------------------------------------

    #[test]
    fn extension_conflicts_custom() {
        let mut svc = LanguageService::new();
        svc.register(LanguageDefinition {
            id: "lang_a".into(), name: "A".into(),
            extensions: vec![".shared".into()], filenames: vec![],
            aliases: vec![], mime_types: vec![], first_line: None,
        });
        svc.register(LanguageDefinition {
            id: "lang_b".into(), name: "B".into(),
            extensions: vec![".shared".into(), ".unique".into()], filenames: vec![],
            aliases: vec![], mime_types: vec![], first_line: None,
        });
        let conflicts = find_extension_conflicts(&svc);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].extension, ".shared");
        assert_eq!(conflicts[0].language_ids.len(), 2);
    }

    // -- parse_shebang ------------------------------------------------------

    #[test]
    fn parse_shebang_env_form() {
        let info = parse_shebang("#!/usr/bin/env python3").unwrap();
        assert_eq!(info.interpreter_name, "python3");
        assert_eq!(info.interpreter_path, "/usr/bin/env");
        assert!(info.args.is_empty());
    }

    #[test]
    fn parse_shebang_direct_form() {
        let info = parse_shebang("#!/bin/bash -x").unwrap();
        assert_eq!(info.interpreter_name, "bash");
        assert_eq!(info.interpreter_path, "/bin/bash");
        assert_eq!(info.args, vec!["-x"]);
    }

    #[test]
    fn parse_shebang_not_a_shebang() {
        assert!(parse_shebang("hello world").is_none());
        assert!(parse_shebang("").is_none());
    }

    // -- find_language_parent -----------------------------------------------

    #[test]
    fn language_inheritance_finds_parent() {
        let svc = make_registry();
        let result = find_language_parent(&svc, "typescript");
        assert!(result.is_some());
        let inh = result.unwrap();
        assert_eq!(inh.language_id, "typescript");
        // TypeScript and JavaScript share // and /* */ comment style
        if let Some(parent) = &inh.parent_id {
            assert!(inh.shared_comment_style);
            assert!(!parent.is_empty());
        }
    }

    #[test]
    fn language_inheritance_unknown_language() {
        let svc = make_registry();
        assert!(find_language_parent(&svc, "nonexistent").is_none());
    }

    // -- LanguageFeatureRegistry ----------------------------------------------

    #[test]
    fn feature_registry_register_and_query() {
        let mut reg = LanguageFeatureRegistry::new();
        reg.register_feature("rust", LanguageFeature::Completion);
        assert!(reg.has_feature("rust", &LanguageFeature::Completion));
    }

    #[test]
    fn feature_registry_missing_feature() {
        let reg = LanguageFeatureRegistry::new();
        assert!(!reg.has_feature("rust", &LanguageFeature::Hover));
    }

    #[test]
    fn feature_registry_unregister() {
        let mut reg = LanguageFeatureRegistry::new();
        reg.register_feature("python", LanguageFeature::Rename);
        assert!(reg.unregister_feature("python", &LanguageFeature::Rename));
        assert!(!reg.has_feature("python", &LanguageFeature::Rename));
    }

    #[test]
    fn feature_registry_unregister_missing() {
        let mut reg = LanguageFeatureRegistry::new();
        assert!(!reg.unregister_feature("go", &LanguageFeature::Formatting));
    }

    #[test]
    fn feature_registry_features_for() {
        let mut reg = LanguageFeatureRegistry::new();
        reg.register_feature("rust", LanguageFeature::Completion);
        reg.register_feature("rust", LanguageFeature::Hover);
        let feats = reg.features_for("rust");
        assert_eq!(feats.len(), 2);
        assert!(feats.contains(&LanguageFeature::Completion));
        assert!(feats.contains(&LanguageFeature::Hover));
    }

    #[test]
    fn feature_registry_features_for_unknown() {
        let reg = LanguageFeatureRegistry::new();
        assert!(reg.features_for("unknown").is_empty());
    }

    #[test]
    fn feature_registry_languages_with_feature() {
        let mut reg = LanguageFeatureRegistry::new();
        reg.register_feature("rust", LanguageFeature::Definition);
        reg.register_feature("python", LanguageFeature::Definition);
        let langs = reg.languages_with_feature(&LanguageFeature::Definition);
        assert_eq!(langs.len(), 2);
    }

    #[test]
    fn feature_registry_feature_count_and_clear() {
        let mut reg = LanguageFeatureRegistry::new();
        reg.register_feature("java", LanguageFeature::CodeAction);
        reg.register_feature("java", LanguageFeature::FoldingRange);
        assert_eq!(reg.feature_count("java"), 2);
        reg.clear_language("java");
        assert_eq!(reg.feature_count("java"), 0);
    }

    // -- language_icon / file_icon_for_extension ------------------------------

    #[test]
    fn icon_rust() {
        assert_eq!(language_icon("rust"), "🦀");
    }

    #[test]
    fn icon_python() {
        assert_eq!(language_icon("python"), "🐍");
    }

    #[test]
    fn icon_javascript() {
        assert_eq!(language_icon("javascript"), "📜");
    }

    #[test]
    fn icon_unknown_default() {
        assert_eq!(language_icon("brainfuck"), "📄");
    }

    #[test]
    fn icon_shell_variants() {
        assert_eq!(language_icon("shell"), "🐚");
        assert_eq!(language_icon("shellscript"), "🐚");
    }

    #[test]
    fn icon_c_cpp() {
        assert_eq!(language_icon("c"), "⚙");
        assert_eq!(language_icon("cpp"), "⚙");
    }

    #[test]
    fn file_icon_rs() {
        assert_eq!(file_icon_for_extension("rs"), "🦀");
    }

    #[test]
    fn file_icon_ts_variants() {
        assert_eq!(file_icon_for_extension("ts"), "📘");
        assert_eq!(file_icon_for_extension("mts"), "📘");
    }

    #[test]
    fn file_icon_unknown() {
        assert_eq!(file_icon_for_extension("xyz"), "📄");
    }

    #[test]
    fn file_icon_html() {
        assert_eq!(file_icon_for_extension("html"), "🌐");
        assert_eq!(file_icon_for_extension("htm"), "🌐");
    }

    // -- LanguageBracketConfig ------------------------------------------------

    #[test]
    fn bracket_config_defaults() {
        let cfg = LanguageBracketConfig::new();
        assert_eq!(cfg.pair_count(), 3);
        assert!(cfg.enabled);
    }

    #[test]
    fn bracket_config_is_open() {
        let cfg = LanguageBracketConfig::new();
        assert!(cfg.is_open_bracket("("));
        assert!(cfg.is_open_bracket("["));
        assert!(cfg.is_open_bracket("{"));
        assert!(!cfg.is_open_bracket(")"));
    }

    #[test]
    fn bracket_config_is_close() {
        let cfg = LanguageBracketConfig::new();
        assert!(cfg.is_close_bracket(")"));
        assert!(!cfg.is_close_bracket("("));
    }

    #[test]
    fn bracket_config_matching_close() {
        let cfg = LanguageBracketConfig::new();
        assert_eq!(cfg.matching_close("("), Some(")"));
        assert_eq!(cfg.matching_close("["), Some("]"));
        assert_eq!(cfg.matching_close("{"), Some("}"));
        assert_eq!(cfg.matching_close("<"), None);
    }

    #[test]
    fn bracket_config_matching_open() {
        let cfg = LanguageBracketConfig::new();
        assert_eq!(cfg.matching_open(")"), Some("("));
        assert_eq!(cfg.matching_open(">"), None);
    }

    #[test]
    fn bracket_config_add_pair() {
        let mut cfg = LanguageBracketConfig::new();
        cfg.add_pair("<", ">");
        assert_eq!(cfg.pair_count(), 4);
        assert!(cfg.is_open_bracket("<"));
        assert_eq!(cfg.matching_close("<"), Some(">"));
    }

    #[test]
    fn bracket_config_with_colorized() {
        let cfg = LanguageBracketConfig::new().with_colorized(false);
        assert!(!cfg.enabled);
    }

    #[test]
    fn bracket_config_default_trait() {
        let cfg = LanguageBracketConfig::default();
        assert_eq!(cfg.pair_count(), 3);
        assert!(cfg.enabled);
    }

    // -- LanguageIdNormalizer ------------------------------------------------

    #[test]
    fn normalizer_resolves_common_aliases() {
        let norm = LanguageIdNormalizer::new();
        assert_eq!(norm.normalize("js"), "javascript");
        assert_eq!(norm.normalize("ts"), "typescript");
        assert_eq!(norm.normalize("py"), "python");
        assert_eq!(norm.normalize("rb"), "ruby");
        assert_eq!(norm.normalize("rs"), "rust");
        assert_eq!(norm.normalize("sh"), "shellscript");
        assert_eq!(norm.normalize("yml"), "yaml");
    }

    #[test]
    fn normalizer_lowercases_and_passes_through() {
        let norm = LanguageIdNormalizer::new();
        assert_eq!(norm.normalize("Rust"), "rust");
        assert_eq!(norm.normalize("PYTHON"), "python");
        assert_eq!(norm.normalize("brainfuck"), "brainfuck");
    }

    #[test]
    fn normalizer_equivalence() {
        let norm = LanguageIdNormalizer::new();
        assert!(norm.are_equivalent("JS", "javascript"));
        assert!(norm.are_equivalent("bash", "sh"));
        assert!(!norm.are_equivalent("rust", "python"));
    }

    #[test]
    fn normalizer_custom_alias() {
        let mut norm = LanguageIdNormalizer::new();
        norm.add_alias("bf", "brainfuck");
        assert_eq!(norm.normalize("bf"), "brainfuck");
        assert!(norm.are_equivalent("BF", "brainfuck"));
    }

    #[test]
    fn normalizer_display() {
        let norm = LanguageIdNormalizer::new();
        let s = format!("{norm}");
        assert!(s.contains("aliases"));
    }

    // -- LanguageDetector ---------------------------------------------------

    #[test]
    fn detector_shebang_python() {
        let det = LanguageDetector::new();
        let content = "#!/usr/bin/env python3\nprint('hello')";
        assert_eq!(det.detect_from_content(content), Some("python".to_string()));
    }

    #[test]
    fn detector_shebang_bash() {
        let det = LanguageDetector::new();
        let content = "#!/bin/bash\necho hello";
        assert_eq!(det.detect_from_content(content), Some("shellscript".to_string()));
    }

    #[test]
    fn detector_xml_declaration() {
        let det = LanguageDetector::new();
        let content = "<?xml version=\"1.0\"?>\n<root/>";
        assert_eq!(det.detect_from_content(content), Some("xml".to_string()));
    }

    #[test]
    fn detector_keyword_heuristic_rust() {
        let det = LanguageDetector::new();
        let content = "use std::io;\nfn main() {\n    let mut x = 5;\n    println!(\"{}\", x);\n}";
        assert_eq!(det.detect_from_content(content), Some("rust".to_string()));
    }

    #[test]
    fn detector_no_match() {
        let det = LanguageDetector::new();
        assert_eq!(det.detect_from_content(""), None);
    }

    #[test]
    fn detector_custom_pattern() {
        let mut det = LanguageDetector::new();
        det.add_magic_pattern(r"^%% Erlang", "erlang").unwrap();
        let content = "%% Erlang module\n-module(test).";
        assert_eq!(det.detect_from_content(content), Some("erlang".to_string()));
    }

    #[test]
    fn detector_display() {
        let det = LanguageDetector::new();
        let s = format!("{det}");
        assert!(s.contains("patterns"));
    }

    // -- LanguageComparison -------------------------------------------------

    #[test]
    fn comparison_rust_vs_cpp() {
        let svc = make_registry();
        let cmp = LanguageComparison::compare(&svc, "rust", "cpp").unwrap();
        assert_eq!(cmp.lang_a, "rust");
        assert_eq!(cmp.lang_b, "cpp");
        // Both use // and /* */, so comment styles should match.
        assert!(cmp.same_line_comment);
        assert!(cmp.same_block_comment);
        assert!(cmp.similarity_score() > 0);
    }

    #[test]
    fn comparison_unknown_returns_none() {
        let svc = make_registry();
        assert!(LanguageComparison::compare(&svc, "rust", "nonexistent").is_none());
    }

    #[test]
    fn comparison_display() {
        let svc = make_registry();
        let cmp = LanguageComparison::compare(&svc, "rust", "python").unwrap();
        let s = format!("{cmp}");
        assert!(s.contains("rust"));
        assert!(s.contains("python"));
        assert!(s.contains('%'));
    }

    // -- LanguageFilter -----------------------------------------------------

    #[test]
    fn filter_with_shebangs() {
        let svc = make_registry();
        let results = LanguageFilter::new(&svc).has_shebangs(true).execute();
        assert!(results.len() >= 4); // python, shellscript, ruby, perl, ...
        // Every result should actually have a first_line pattern
        for id in &results {
            let lang = svc.get_language(id).unwrap();
            assert!(lang.first_line.is_some(), "{id} should have first_line");
        }
    }

    #[test]
    fn filter_no_block_comments() {
        let svc = make_registry();
        let results = LanguageFilter::new(&svc).has_block_comments(false).execute();
        for id in &results {
            if let Some(cfg) = svc.get_edit_config(id) {
                assert!(cfg.comments.block_comment.is_none(), "{id} should lack block comments");
            }
        }
    }

    #[test]
    fn filter_extension_contains() {
        let svc = make_registry();
        let results = LanguageFilter::new(&svc).extension_contains(".rs").execute();
        assert!(results.contains(&"rust".to_string()));
    }

    #[test]
    fn filter_chained() {
        let svc = make_registry();
        let results = LanguageFilter::new(&svc)
            .has_block_comments(true)
            .has_line_comments(true)
            .min_extensions(1)
            .execute();
        // C-family languages should appear
        assert!(!results.is_empty());
    }

    #[test]
    fn filter_display() {
        let svc = make_registry();
        let f = LanguageFilter::new(&svc).has_shebangs(true).has_block_comments(false);
        let s = format!("{f}");
        assert!(s.contains("shebangs"));
        assert!(s.contains("block_comments"));
    }

    // -- Display impls for existing types -----------------------------------

    #[test]
    fn display_language_stats() {
        let svc = make_registry();
        let stats = compute_language_stats(&svc);
        let s = format!("{stats}");
        assert!(s.contains("languages"));
        assert!(s.contains("extensions"));
    }

    #[test]
    fn display_shebang_info() {
        let info = parse_shebang("#!/usr/bin/env python3 -u").unwrap();
        let s = format!("{info}");
        assert_eq!(s, "python3 -u");
    }

    #[test]
    fn display_bracket_config() {
        let cfg = LanguageBracketConfig::new();
        let s = format!("{cfg}");
        assert!(s.contains("3 bracket pairs"));
    }

    // -- From impl for LanguageBracketConfig --------------------------------

    #[test]
    fn bracket_config_from_str() {
        let cfg = LanguageBracketConfig::from("(),[],{}");
        assert_eq!(cfg.pair_count(), 3);
        assert!(cfg.is_open_bracket("("));
        assert!(cfg.is_close_bracket("]"));
        assert_eq!(cfg.matching_close("{"), Some("}"));
    }

    // -- LanguageAutoDetector -----------------------------------------------

    #[test]
    fn auto_detector_shebang_env_python() {
        let det = LanguageAutoDetector::new();
        assert_eq!(
            det.detect_by_first_line("#!/usr/bin/env python3"),
            Some("python".to_string()),
        );
    }

    #[test]
    fn auto_detector_shebang_direct_bash() {
        let det = LanguageAutoDetector::new();
        assert_eq!(
            det.detect_by_first_line("#!/bin/bash"),
            Some("shellscript".to_string()),
        );
    }

    #[test]
    fn auto_detector_xml_declaration() {
        let det = LanguageAutoDetector::new();
        let content = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root/>";
        assert_eq!(det.detect_by_content(content), Some("xml".to_string()));
    }

    #[test]
    fn auto_detector_custom_pattern() {
        let mut det = LanguageAutoDetector::new();
        det.add_pattern(r"^%!PS", "postscript").unwrap();
        assert_eq!(
            det.detect_by_first_line("%!PS-Adobe-3.0"),
            Some("postscript".to_string()),
        );
    }

    #[test]
    fn auto_detector_no_match() {
        let det = LanguageAutoDetector::new();
        assert_eq!(det.detect_by_first_line("hello world"), None);
    }

    // -- LanguageConfigurationResolver --------------------------------------

    #[test]
    fn resolver_rust_comments() {
        let svc = make_registry();
        let resolver = LanguageConfigurationResolver::new(&svc);
        let cfg = resolver.resolve("rust");
        assert_eq!(cfg.line_comment.as_deref(), Some("//"));
        assert_eq!(cfg.block_comment_start.as_deref(), Some("/*"));
        assert_eq!(cfg.block_comment_end.as_deref(), Some("*/"));
        assert!(!cfg.auto_closing_pairs.is_empty());
    }

    #[test]
    fn resolver_unknown_language_defaults() {
        let svc = make_registry();
        let resolver = LanguageConfigurationResolver::new(&svc);
        let cfg = resolver.resolve("nonexistent");
        assert!(cfg.line_comment.is_none());
        assert!(cfg.block_comment_start.is_none());
        assert!(cfg.auto_closing_pairs.is_empty());
    }

    #[test]
    fn resolver_has_comment_helpers() {
        let svc = make_registry();
        let resolver = LanguageConfigurationResolver::new(&svc);
        assert!(resolver.has_line_comment("rust"));
        assert!(resolver.has_block_comment("rust"));
        assert!(!resolver.has_line_comment("html"));
    }

    // -- LanguageScope ------------------------------------------------------

    #[test]
    fn scope_host_language() {
        let scope = LanguageScope::new("html");
        assert_eq!(scope.current_language(), "html");
        assert_eq!(scope.depth(), 1);
        assert!(!scope.is_embedded());
    }

    #[test]
    fn scope_push_and_pop() {
        let mut scope = LanguageScope::new("html");
        scope.push_scope("javascript");
        assert_eq!(scope.current_language(), "javascript");
        assert_eq!(scope.depth(), 2);
        assert!(scope.is_embedded());

        scope.push_scope("css");
        assert_eq!(scope.current_language(), "css");
        assert_eq!(scope.depth(), 3);

        assert_eq!(scope.pop_scope(), Some("css".to_string()));
        assert_eq!(scope.current_language(), "javascript");

        assert_eq!(scope.pop_scope(), Some("javascript".to_string()));
        assert_eq!(scope.current_language(), "html");

        // Cannot pop the host language
        assert_eq!(scope.pop_scope(), None);
        assert_eq!(scope.depth(), 1);
    }

    // -- LanguageAliasMapper ------------------------------------------------

    #[test]
    fn alias_mapper_resolve() {
        let mut mapper = LanguageAliasMapper::new();
        mapper.add_alias("js", "javascript");
        mapper.add_alias("ts", "typescript");
        assert_eq!(mapper.resolve("js"), "javascript");
        assert_eq!(mapper.resolve("ts"), "typescript");
        // Unknown names pass through unchanged
        assert_eq!(mapper.resolve("rust"), "rust");
    }

    #[test]
    fn alias_mapper_aliases_for() {
        let mut mapper = LanguageAliasMapper::new();
        mapper.add_alias("js", "javascript");
        mapper.add_alias("ecmascript", "javascript");
        let aliases = mapper.aliases_for("javascript");
        assert_eq!(aliases.len(), 2);
        assert!(aliases.contains(&"js".to_string()));
        assert!(aliases.contains(&"ecmascript".to_string()));
    }

    #[test]
    fn alias_mapper_equivalence() {
        let mut mapper = LanguageAliasMapper::new();
        mapper.add_alias("js", "javascript");
        mapper.add_alias("ecma", "javascript");
        assert!(mapper.are_equivalent("js", "ecma"));
        assert!(!mapper.are_equivalent("js", "python"));
    }

    #[test]
    fn langc_lru_insert_get() {
        let mut c = LangCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn langc_lru_eviction() {
        let mut c = LangCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn langc_lru_hit_ratio() {
        let mut c = LangCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn langc_lru_clear() {
        let mut c = LangCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn langc_lru_remove() {
        let mut c = LangCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn langc_lru_peek() {
        let mut c = LangCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn langb_builder_valid() {
        let cfg = LangBBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn langb_builder_empty_name() {
        let r = LangBBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn langb_builder_bad_priority() {
        assert!(LangBBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn langb_builder_zero_max() {
        assert!(LangBBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn langb_cfg_merge() {
        let mut a = LangBBuilder::new("a").property("x", "1").build().unwrap();
        let b = LangBBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn langb_cfg_display() {
        let cfg = LangBBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    #[test]
    fn languages_entry_creation() {
        let e = LanguagesEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn languages_entry_with_priority() {
        let e = LanguagesEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn languages_entry_metadata() {
        let e = LanguagesEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn languages_entry_remove_meta() {
        let mut e = LanguagesEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn languages_entry_activate_deactivate() {
        let mut e = LanguagesEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn languages_config_add_sorted() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("lo", "Lo").with_priority(1));
        c.add(LanguagesEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn languages_config_capacity() {
        let mut c = LanguagesConfig::new(1);
        assert!(c.add(LanguagesEntry::new("a", "A")));
        assert!(!c.add(LanguagesEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn languages_config_remove() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn languages_config_get() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn languages_config_active_entries() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("a", "A"));
        c.add(LanguagesEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn languages_config_enable_disable() {
        let mut c = LanguagesConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn languages_config_clear() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn languages_config_find_by_label() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn languages_config_top_n() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("a", "A").with_priority(1));
        c.add(LanguagesEntry::new("b", "B").with_priority(2));
        c.add(LanguagesEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn languages_config_deactivate_activate_all() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("a", "A"));
        c.add(LanguagesEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn languages_config_highest_priority() {
        let mut c = LanguagesConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(LanguagesEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn languages_config_contains() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn languages_config_labels() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("a", "Alpha"));
        c.add(LanguagesEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn languages_config_drain_inactive() {
        let mut c = LanguagesConfig::new(10);
        c.add(LanguagesEntry::new("a", "A"));
        c.add(LanguagesEntry::new("b", "B"));
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


    // xa_ extended tests for languages
    #[test]
    fn xa_languages_ring_new() {
        let rb = super::XaLanguagesRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_languages_ring_push_len() {
        let mut rb = super::XaLanguagesRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_languages_ring_wrap() {
        let mut rb = super::XaLanguagesRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_languages_ring_mean_empty() {
        let rb = super::XaLanguagesRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_languages_ring_mean_values() {
        let mut rb = super::XaLanguagesRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_languages_ring_min_max() {
        let mut rb = super::XaLanguagesRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_languages_ring_iter() {
        let mut rb = super::XaLanguagesRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_languages_counter_new() {
        let c = super::XaLanguagesCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_languages_counter_inc() {
        let mut c = super::XaLanguagesCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_languages_counter_inc_by() {
        let mut c = super::XaLanguagesCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_languages_counter_reset() {
        let mut c = super::XaLanguagesCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_languages_counter_clear() {
        let mut c = super::XaLanguagesCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_languages_counter_default() {
        let c = super::XaLanguagesCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 108 ----

    #[test]
    fn xc_108_pool_new_empty() {
        let pool: super::Xc108Pool<i32> = super::Xc108Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_108_pool_release_acquire() {
        let mut pool = super::Xc108Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_108_pool_acquire_empty() {
        let mut pool: super::Xc108Pool<i32> = super::Xc108Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_108_pool_full() {
        let mut pool = super::Xc108Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_108_pool_drain() {
        let mut pool = super::Xc108Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_108_pool_stats() {
        let mut pool = super::Xc108Pool::new(8);
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
    fn xc_108_pool_clear() {
        let mut pool = super::Xc108Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_108_pool_shrink() {
        let mut pool = super::Xc108Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_108_pool_default() {
        let pool: super::Xc108Pool<String> = super::Xc108Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_108_pool_extend() {
        let mut pool = super::Xc108Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_108_pool_retain() {
        let mut pool = super::Xc108Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_108_scheduler_round_robin() {
        let mut sched = super::Xc108Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_108_scheduler_empty() {
        let mut sched = super::Xc108Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_108_scheduler_reset() {
        let mut sched = super::Xc108Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_108_scheduler_add_remove() {
        let mut sched = super::Xc108Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_108_scheduler_targets() {
        let sched = super::Xc108Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_108_hash_empty() {
        assert_eq!(super::xc_108_hash(b""), 5381);
    }

    #[test]
    fn xc_108_hash_data() {
        let h = super::xc_108_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_108_hash(b"hello"), h);
    }

    #[test]
    fn xc_108_reverse_str() {
        assert_eq!(super::xc_108_reverse("abc"), "cba");
        assert_eq!(super::xc_108_reverse(""), "");
    }


    // --- xd_75 deepening tests ---

    #[test]
    fn xd_75_sm_initial_state() {
        let sm = Xd75StateMachine::new();
        assert_eq!(sm.current_state(), Xd75State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_75_sm_valid_idle_to_running() {
        let mut sm = Xd75StateMachine::new();
        assert!(sm.transition(Xd75State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd75State::Running);
    }

    #[test]
    fn xd_75_sm_valid_running_to_paused() {
        let mut sm = Xd75StateMachine::new();
        sm.transition(Xd75State::Running).unwrap();
        assert!(sm.transition(Xd75State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd75State::Paused);
    }

    #[test]
    fn xd_75_sm_valid_running_to_done() {
        let mut sm = Xd75StateMachine::new();
        sm.transition(Xd75State::Running).unwrap();
        assert!(sm.transition(Xd75State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd75State::Done);
    }

    #[test]
    fn xd_75_sm_valid_paused_to_running() {
        let mut sm = Xd75StateMachine::new();
        sm.transition(Xd75State::Running).unwrap();
        sm.transition(Xd75State::Paused).unwrap();
        assert!(sm.transition(Xd75State::Running).is_ok());
    }

    #[test]
    fn xd_75_sm_valid_done_to_idle() {
        let mut sm = Xd75StateMachine::new();
        sm.transition(Xd75State::Running).unwrap();
        sm.transition(Xd75State::Done).unwrap();
        assert!(sm.transition(Xd75State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd75State::Idle);
    }

    #[test]
    fn xd_75_sm_invalid_idle_to_done() {
        let mut sm = Xd75StateMachine::new();
        assert!(sm.transition(Xd75State::Done).is_err());
    }

    #[test]
    fn xd_75_sm_invalid_idle_to_paused() {
        let mut sm = Xd75StateMachine::new();
        assert!(sm.transition(Xd75State::Paused).is_err());
    }

    #[test]
    fn xd_75_sm_history_tracking() {
        let mut sm = Xd75StateMachine::new();
        sm.transition(Xd75State::Running).unwrap();
        sm.transition(Xd75State::Paused).unwrap();
        sm.transition(Xd75State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd75State::Idle);
        assert_eq!(sm.history()[0].to, Xd75State::Running);
        assert_eq!(sm.history()[1].from, Xd75State::Running);
        assert_eq!(sm.history()[2].to, Xd75State::Done);
    }

    #[test]
    fn xd_75_sm_serialize_deserialize() {
        let mut sm = Xd75StateMachine::new();
        sm.transition(Xd75State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd75StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd75State::Running));
    }

    #[test]
    fn xd_75_sm_deserialize_invalid() {
        assert_eq!(Xd75StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_75_sm_reset() {
        let mut sm = Xd75StateMachine::new();
        sm.transition(Xd75State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd75State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_75_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd75EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd75Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_75_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd75EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd75Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd75Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_75_bus_unsubscribe() {
        let mut bus = Xd75EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_75_event_kind_and_payload() {
        let e = Xd75Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd75Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_75_bus_clear_history() {
        let mut bus = Xd75EventBus::new();
        bus.publish(Xd75Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_75_sm_step_counter_increments() {
        let mut sm = Xd75StateMachine::new();
        sm.transition(Xd75State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd75State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #93 --

    #[test]
    fn xf93_trie_insert_search() {
        let mut t = Xf93Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf93_trie_starts_with() {
        let mut t = Xf93Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf93_trie_remove() {
        let mut t = Xf93Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf93_trie_word_count() {
        let mut t = Xf93Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf93_trie_longest_prefix() {
        let mut t = Xf93Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf93_trie_all_words() {
        let mut t = Xf93Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf93_trie_autocomplete() {
        let mut t = Xf93Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf93_trie_empty_search() {
        let t = Xf93Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf93_bloom_add_contains() {
        let mut bf = Xf93BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf93_bloom_probably_absent() {
        let bf = Xf93BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf93_bloom_false_positive_rate() {
        let mut bf = Xf93BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf93_bloom_clear() {
        let mut bf = Xf93BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf93_bloom_union() {
        let mut a = Xf93BloomFilter::xf_new(512, 2);
        let mut b = Xf93BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf93_bloom_intersection_estimate() {
        let mut a = Xf93BloomFilter::xf_new(512, 2);
        let mut b = Xf93BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf93_bloom_union_size_mismatch() {
        let a = Xf93BloomFilter::xf_new(256, 2);
        let b = Xf93BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}