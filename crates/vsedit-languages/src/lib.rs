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
}
