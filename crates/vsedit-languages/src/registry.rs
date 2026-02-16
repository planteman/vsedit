//! Language service — registration and lookup.

use std::collections::HashMap;

use regex::Regex;

use crate::config::LanguageEditConfig;
use crate::definition::LanguageDefinition;
use crate::defaults::build_edit_configs;

/// Manages registered languages with indexes for fast lookup and per-language
/// editing configuration.
pub struct LanguageService {
    languages: Vec<LanguageDefinition>,
    ext_index: HashMap<String, usize>,
    filename_index: HashMap<String, usize>,
    edit_configs: HashMap<String, LanguageEditConfig>,
}

impl LanguageService {
    /// Create an empty service.
    pub fn new() -> Self {
        Self {
            languages: Vec::new(),
            ext_index: HashMap::new(),
            filename_index: HashMap::new(),
            edit_configs: HashMap::new(),
        }
    }

    /// Register a language definition.
    ///
    /// Indexes are updated for all extensions and filenames declared in the
    /// definition. Later registrations overwrite earlier ones for the same
    /// extension or filename.
    pub fn register(&mut self, config: LanguageDefinition) {
        let idx = self.languages.len();

        for ext in &config.extensions {
            self.ext_index.insert(ext.to_lowercase(), idx);
        }
        for name in &config.filenames {
            self.filename_index.insert(name.clone(), idx);
        }

        self.languages.push(config);
    }

    /// Register an editing configuration for a language id.
    pub fn register_edit_config(&mut self, language_id: &str, config: LanguageEditConfig) {
        self.edit_configs.insert(language_id.to_string(), config);
    }

    /// Register all built-in editing configurations.
    pub fn register_default_edit_configs(&mut self) {
        for (id, cfg) in build_edit_configs() {
            self.edit_configs.insert(id, cfg);
        }
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

    /// Get a language definition by its id.
    pub fn get_language(&self, id: &str) -> Option<&LanguageDefinition> {
        self.languages.iter().find(|lang| lang.id == id)
    }

    /// Get editing configuration for a language id.
    pub fn get_edit_config(&self, id: &str) -> Option<&LanguageEditConfig> {
        self.edit_configs.get(id)
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

impl Default for LanguageService {
    fn default() -> Self {
        Self::new()
    }
}
