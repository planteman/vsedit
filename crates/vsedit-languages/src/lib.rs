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
// Tests
// ---------------------------------------------------------------------------

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
}
