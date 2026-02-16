//! Language registration service.

/// Metadata for a single language.
#[derive(Debug, Clone)]
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
}

impl std::fmt::Display for LanguageInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.id)
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
}
