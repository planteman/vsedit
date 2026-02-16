//! Inline code lens decorations.

/// A command associated with a code lens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub title: String,
    pub command_id: String,
    pub tooltip: String,
    pub arguments: Vec<String>,
}

/// A code lens representing a command anchored to a source range.
///
/// A code lens may be unresolved (no command yet) when first returned by a
/// provider. The service calls [`CodeLensProvider::resolve_code_lens`] to
/// fill in the command lazily.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLens {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub command: Option<Command>,
    pub data: String,
}

impl CodeLens {
    /// Create a new unresolved code lens for the given range.
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
            command: None,
            data: String::new(),
        }
    }

    /// Returns `true` if a command has been attached.
    pub fn is_resolved(&self) -> bool {
        self.command.is_some()
    }
}

/// Provider that supplies code lenses for a document.
pub trait CodeLensProvider: Send + Sync {
    /// Return all code lenses for the given document URI.
    ///
    /// Lenses may be returned unresolved (without a command); the service will
    /// call [`resolve_code_lens`](CodeLensProvider::resolve_code_lens) later.
    fn provide_code_lenses(&self, uri: &str) -> Vec<CodeLens>;

    /// Fill in the command for an unresolved code lens.
    ///
    /// The default implementation returns the lens unchanged.
    fn resolve_code_lens(&self, lens: CodeLens) -> CodeLens {
        lens
    }
}

/// Service that manages registered [`CodeLensProvider`]s and collects lenses.
pub struct CodeLensService {
    providers: Vec<Box<dyn CodeLensProvider>>,
}

impl CodeLensService {
    /// Create an empty service with no providers.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a code lens provider.
    pub fn register(&mut self, provider: impl CodeLensProvider + 'static) {
        self.providers.push(Box::new(provider));
    }

    /// Returns the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Collect code lenses from all providers for the given URI.
    pub fn provide_code_lenses(&self, uri: &str) -> Vec<CodeLens> {
        self.providers
            .iter()
            .flat_map(|p| p.provide_code_lenses(uri))
            .collect()
    }

    /// Resolve all unresolved lenses using the provider that produced them.
    ///
    /// This convenience method resolves every lens by iterating providers in
    /// registration order and calling `resolve_code_lens` on the first
    /// provider that returns a resolved result.
    pub fn resolve_all(&self, lenses: Vec<CodeLens>) -> Vec<CodeLens> {
        lenses
            .into_iter()
            .map(|lens| {
                if lens.is_resolved() {
                    return lens;
                }
                for provider in &self.providers {
                    let resolved = provider.resolve_code_lens(lens.clone());
                    if resolved.is_resolved() {
                        return resolved;
                    }
                }
                lens
            })
            .collect()
    }
}

impl Default for CodeLensService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Test providers -----------------------------------------------------

    struct ReferenceCountProvider;

    impl CodeLensProvider for ReferenceCountProvider {
        fn provide_code_lenses(&self, _uri: &str) -> Vec<CodeLens> {
            vec![
                CodeLens {
                    data: "ref_count".into(),
                    ..CodeLens::new(0, 0, 0, 10)
                },
                CodeLens {
                    data: "ref_count".into(),
                    ..CodeLens::new(5, 0, 5, 15)
                },
            ]
        }

        fn resolve_code_lens(&self, mut lens: CodeLens) -> CodeLens {
            if lens.data == "ref_count" {
                lens.command = Some(Command {
                    title: "3 references".into(),
                    command_id: "editor.showReferences".into(),
                    tooltip: "Show all references".into(),
                    arguments: vec![],
                });
            }
            lens
        }
    }

    struct RunTestProvider;

    impl CodeLensProvider for RunTestProvider {
        fn provide_code_lenses(&self, uri: &str) -> Vec<CodeLens> {
            if uri.ends_with("_test.rs") {
                vec![CodeLens {
                    command: Some(Command {
                        title: "▶ Run Test".into(),
                        command_id: "test.run".into(),
                        tooltip: "Run this test".into(),
                        arguments: vec![uri.to_string()],
                    }),
                    ..CodeLens::new(1, 0, 1, 20)
                }]
            } else {
                vec![]
            }
        }
    }

    // -- Tests --------------------------------------------------------------

    #[test]
    fn code_lens_new_is_unresolved() {
        let lens = CodeLens::new(10, 0, 10, 25);
        assert!(!lens.is_resolved());
        assert_eq!(lens.start_line, 10);
        assert_eq!(lens.end_col, 25);
        assert!(lens.data.is_empty());
    }

    #[test]
    fn service_collects_from_multiple_providers() {
        let mut service = CodeLensService::new();
        service.register(ReferenceCountProvider);
        service.register(RunTestProvider);

        assert_eq!(service.provider_count(), 2);

        let lenses = service.provide_code_lenses("main_test.rs");
        assert_eq!(lenses.len(), 3);
    }

    #[test]
    fn resolve_fills_in_command() {
        let mut service = CodeLensService::new();
        service.register(ReferenceCountProvider);

        let lenses = service.provide_code_lenses("main.rs");
        assert!(lenses.iter().all(|l| !l.is_resolved()));

        let resolved = service.resolve_all(lenses);
        assert!(resolved.iter().all(|l| l.is_resolved()));
        assert_eq!(resolved[0].command.as_ref().unwrap().title, "3 references");
    }

    #[test]
    fn provider_filters_by_uri() {
        let mut service = CodeLensService::new();
        service.register(RunTestProvider);

        let test_lenses = service.provide_code_lenses("foo_test.rs");
        assert_eq!(test_lenses.len(), 1);
        assert!(test_lenses[0].is_resolved());

        let src_lenses = service.provide_code_lenses("foo.rs");
        assert!(src_lenses.is_empty());
    }

    #[test]
    fn empty_service_returns_no_lenses() {
        let service = CodeLensService::default();
        assert_eq!(service.provider_count(), 0);
        assert!(service.provide_code_lenses("any.rs").is_empty());
    }

    #[test]
    fn resolve_skips_already_resolved() {
        let mut service = CodeLensService::new();
        service.register(RunTestProvider);

        let lenses = service.provide_code_lenses("x_test.rs");
        assert!(lenses[0].is_resolved());
        let original_title = lenses[0].command.as_ref().unwrap().title.clone();

        let resolved = service.resolve_all(lenses);
        assert_eq!(resolved[0].command.as_ref().unwrap().title, original_title);
    }
}
