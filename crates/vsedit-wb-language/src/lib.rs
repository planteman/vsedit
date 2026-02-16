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
}
