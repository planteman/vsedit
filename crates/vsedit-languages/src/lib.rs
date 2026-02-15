//! Language registration and detection.
//!
//! Manages language definitions and provides lookup by file extension,
//! filename, MIME type, and first-line content (e.g. shebangs).
//!
//! # Key types
//!
//! - [`LanguageConfiguration`] — describes a single language.
//! - [`LanguageRegistry`] — stores languages and provides fast lookups.
//! - [`register_default_languages`] — registers ~20 common languages.

use std::collections::HashMap;

use regex::Regex;

/// A language identifier string (e.g. `"rust"`, `"typescript"`).
pub type LanguageId = String;

// ---------------------------------------------------------------------------
// LanguageConfiguration
// ---------------------------------------------------------------------------

/// Configuration for a registered language.
#[derive(Debug, Clone)]
pub struct LanguageConfiguration {
    /// Unique identifier (e.g. `"rust"`).
    pub id: String,
    /// Human-readable name (e.g. `"Rust"`).
    pub name: String,
    /// File extensions including the dot (e.g. `[".rs"]`).
    pub extensions: Vec<String>,
    /// Exact filenames (e.g. `["Makefile"]`).
    pub filenames: Vec<String>,
    /// Alternative names (e.g. `["Rust", "rust"]`).
    pub aliases: Vec<String>,
    /// Associated MIME types (e.g. `["text/x-rust"]`).
    pub mime_types: Vec<String>,
    /// Regex for first-line detection (e.g. `"^#!.*python"`).
    pub first_line: Option<String>,
}

// ---------------------------------------------------------------------------
// LanguageRegistry
// ---------------------------------------------------------------------------

/// Manages registered languages with indexes for fast lookup.
pub struct LanguageRegistry {
    languages: Vec<LanguageConfiguration>,
    ext_index: HashMap<String, usize>,
    filename_index: HashMap<String, usize>,
}

impl LanguageRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            languages: Vec::new(),
            ext_index: HashMap::new(),
            filename_index: HashMap::new(),
        }
    }

    /// Register a language configuration.
    ///
    /// Indexes are updated for all extensions and filenames declared in the
    /// configuration. Later registrations overwrite earlier ones for the same
    /// extension or filename.
    pub fn register(&mut self, config: LanguageConfiguration) {
        let idx = self.languages.len();

        for ext in &config.extensions {
            self.ext_index.insert(ext.to_lowercase(), idx);
        }
        for name in &config.filenames {
            self.filename_index.insert(name.clone(), idx);
        }

        self.languages.push(config);
    }

    /// Look up a language by file extension (e.g. `".rs"`).
    pub fn get_language_id_by_extension(&self, ext: &str) -> Option<&str> {
        self.ext_index
            .get(&ext.to_lowercase())
            .map(|&idx| self.languages[idx].id.as_str())
    }

    /// Look up a language by exact filename (e.g. `"Makefile"`).
    pub fn get_language_id_by_filename(&self, name: &str) -> Option<&str> {
        self.filename_index
            .get(name)
            .map(|&idx| self.languages[idx].id.as_str())
    }

    /// Look up a language by MIME type (e.g. `"text/x-rust"`).
    pub fn get_language_id_by_mime(&self, mime: &str) -> Option<&str> {
        self.languages
            .iter()
            .find(|lang| lang.mime_types.iter().any(|m| m == mime))
            .map(|lang| lang.id.as_str())
    }

    /// Look up a language by matching `first_line` regex against the given line.
    pub fn get_language_id_by_first_line(&self, first_line: &str) -> Option<&str> {
        for lang in &self.languages {
            if let Some(pattern) = &lang.first_line {
                if let Ok(re) = Regex::new(pattern) {
                    if re.is_match(first_line) {
                        return Some(&lang.id);
                    }
                }
            }
        }
        None
    }

    /// Get a language configuration by its id.
    pub fn get_language(&self, id: &str) -> Option<&LanguageConfiguration> {
        self.languages.iter().find(|lang| lang.id == id)
    }

    /// Return the ids of all registered languages.
    pub fn get_registered_language_ids(&self) -> Vec<&str> {
        self.languages.iter().map(|lang| lang.id.as_str()).collect()
    }

    /// Guess the language for a file, trying filename, extension, and
    /// optionally the first line of content.
    pub fn guess_language_id(&self, filename: &str, first_line: Option<&str>) -> Option<&str> {
        // 1. Exact filename match
        let basename = filename.rsplit('/').next().unwrap_or(filename);
        if let Some(id) = self.get_language_id_by_filename(basename) {
            return Some(id);
        }

        // 2. Extension match
        if let Some(dot_pos) = basename.rfind('.') {
            let ext = &basename[dot_pos..];
            if let Some(id) = self.get_language_id_by_extension(ext) {
                return Some(id);
            }
        }

        // 3. First-line match
        if let Some(line) = first_line {
            if let Some(id) = self.get_language_id_by_first_line(line) {
                return Some(id);
            }
        }

        None
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Default language registrations
// ---------------------------------------------------------------------------

/// Register ~20 common languages with the given registry.
pub fn register_default_languages(registry: &mut LanguageRegistry) {
    registry.register(LanguageConfiguration {
        id: "rust".into(),
        name: "Rust".into(),
        extensions: vec![".rs".into()],
        filenames: vec![],
        aliases: vec!["Rust".into(), "rust".into()],
        mime_types: vec!["text/x-rust".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "typescript".into(),
        name: "TypeScript".into(),
        extensions: vec![".ts".into(), ".mts".into(), ".cts".into()],
        filenames: vec![],
        aliases: vec!["TypeScript".into(), "ts".into()],
        mime_types: vec!["text/typescript".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "javascript".into(),
        name: "JavaScript".into(),
        extensions: vec![".js".into(), ".mjs".into(), ".cjs".into()],
        filenames: vec![],
        aliases: vec!["JavaScript".into(), "js".into()],
        mime_types: vec!["text/javascript".into()],
        first_line: Some(r"^#!.*\bnode\b".into()),
    });

    registry.register(LanguageConfiguration {
        id: "python".into(),
        name: "Python".into(),
        extensions: vec![".py".into(), ".pyi".into()],
        filenames: vec![],
        aliases: vec!["Python".into(), "py".into()],
        mime_types: vec!["text/x-python".into()],
        first_line: Some(r"^#!.*\bpython[23]?\b".into()),
    });

    registry.register(LanguageConfiguration {
        id: "go".into(),
        name: "Go".into(),
        extensions: vec![".go".into()],
        filenames: vec![],
        aliases: vec!["Go".into(), "golang".into()],
        mime_types: vec!["text/x-go".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "java".into(),
        name: "Java".into(),
        extensions: vec![".java".into()],
        filenames: vec![],
        aliases: vec!["Java".into()],
        mime_types: vec!["text/x-java".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "c".into(),
        name: "C".into(),
        extensions: vec![".c".into(), ".h".into()],
        filenames: vec![],
        aliases: vec!["C".into()],
        mime_types: vec!["text/x-c".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "cpp".into(),
        name: "C++".into(),
        extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".hpp".into(), ".hxx".into()],
        filenames: vec![],
        aliases: vec!["C++".into(), "cpp".into()],
        mime_types: vec!["text/x-c++src".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "csharp".into(),
        name: "C#".into(),
        extensions: vec![".cs".into()],
        filenames: vec![],
        aliases: vec!["C#".into(), "csharp".into()],
        mime_types: vec!["text/x-csharp".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "html".into(),
        name: "HTML".into(),
        extensions: vec![".html".into(), ".htm".into()],
        filenames: vec![],
        aliases: vec!["HTML".into()],
        mime_types: vec!["text/html".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "css".into(),
        name: "CSS".into(),
        extensions: vec![".css".into()],
        filenames: vec![],
        aliases: vec!["CSS".into()],
        mime_types: vec!["text/css".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "json".into(),
        name: "JSON".into(),
        extensions: vec![".json".into()],
        filenames: vec![],
        aliases: vec!["JSON".into()],
        mime_types: vec!["application/json".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "jsonc".into(),
        name: "JSON with Comments".into(),
        extensions: vec![".jsonc".into()],
        filenames: vec![],
        aliases: vec!["JSONC".into(), "jsonc".into()],
        mime_types: vec!["application/json".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "yaml".into(),
        name: "YAML".into(),
        extensions: vec![".yml".into(), ".yaml".into()],
        filenames: vec![],
        aliases: vec!["YAML".into(), "yml".into()],
        mime_types: vec!["text/yaml".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "toml".into(),
        name: "TOML".into(),
        extensions: vec![".toml".into()],
        filenames: vec!["Cargo.toml".into()],
        aliases: vec!["TOML".into()],
        mime_types: vec!["text/x-toml".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "markdown".into(),
        name: "Markdown".into(),
        extensions: vec![".md".into(), ".markdown".into()],
        filenames: vec![],
        aliases: vec!["Markdown".into(), "md".into()],
        mime_types: vec!["text/markdown".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "shellscript".into(),
        name: "Shell Script".into(),
        extensions: vec![".sh".into(), ".bash".into(), ".zsh".into()],
        filenames: vec![".bashrc".into(), ".zshrc".into(), ".profile".into()],
        aliases: vec!["Shell".into(), "bash".into(), "sh".into()],
        mime_types: vec!["text/x-shellscript".into()],
        first_line: Some(r"^#!.*\b(bash|sh|zsh)\b".into()),
    });

    registry.register(LanguageConfiguration {
        id: "xml".into(),
        name: "XML".into(),
        extensions: vec![".xml".into(), ".xsl".into(), ".xsd".into()],
        filenames: vec![],
        aliases: vec!["XML".into()],
        mime_types: vec!["text/xml".into(), "application/xml".into()],
        first_line: Some(r"^<\?xml\b".into()),
    });

    registry.register(LanguageConfiguration {
        id: "sql".into(),
        name: "SQL".into(),
        extensions: vec![".sql".into()],
        filenames: vec![],
        aliases: vec!["SQL".into()],
        mime_types: vec!["text/x-sql".into()],
        first_line: None,
    });

    registry.register(LanguageConfiguration {
        id: "dockerfile".into(),
        name: "Dockerfile".into(),
        extensions: vec![".dockerfile".into()],
        filenames: vec!["Dockerfile".into(), "Containerfile".into()],
        aliases: vec!["Dockerfile".into(), "docker".into()],
        mime_types: vec!["text/x-dockerfile".into()],
        first_line: None,
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> LanguageRegistry {
        let mut reg = LanguageRegistry::new();
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
        assert_eq!(ids.len(), 20);
    }

    // -- Custom registration ------------------------------------------------

    #[test]
    fn custom_language_registration() {
        let mut reg = LanguageRegistry::new();
        reg.register(LanguageConfiguration {
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
        let reg = LanguageRegistry::default();
        assert!(reg.get_registered_language_ids().is_empty());
    }
}
