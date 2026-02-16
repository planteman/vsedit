//! Inline code lens decorations.

use std::fmt;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during code lens operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeLensError {
    /// The range is invalid (start is after end).
    InvalidRange {
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    },
    /// A required field was missing when building a command.
    MissingField(&'static str),
    /// The lens could not be resolved by any provider.
    UnresolvedLens { data: String },
}

impl fmt::Display for CodeLensError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange {
                start_line,
                start_col,
                end_line,
                end_col,
            } => write!(
                f,
                "invalid range: ({start_line}:{start_col}) > ({end_line}:{end_col})"
            ),
            Self::MissingField(name) => write!(f, "missing required field: {name}"),
            Self::UnresolvedLens { data } => {
                write!(f, "lens with data '{data}' could not be resolved")
            }
        }
    }
}

impl std::error::Error for CodeLensError {}

// ---------------------------------------------------------------------------
// Command & CommandBuilder
// ---------------------------------------------------------------------------

/// A command associated with a code lens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub title: String,
    pub command_id: String,
    pub tooltip: String,
    pub arguments: Vec<String>,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.command_id, self.title)
    }
}

/// Builder for constructing a [`Command`] with validation.
#[derive(Debug, Clone, Default)]
pub struct CommandBuilder {
    title: Option<String>,
    command_id: Option<String>,
    tooltip: Option<String>,
    arguments: Vec<String>,
}

impl CommandBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn command_id(mut self, id: impl Into<String>) -> Self {
        self.command_id = Some(id.into());
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn argument(mut self, arg: impl Into<String>) -> Self {
        self.arguments.push(arg.into());
        self
    }

    pub fn arguments(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.arguments.extend(args.into_iter().map(Into::into));
        self
    }

    /// Build the command, returning an error if required fields are missing.
    pub fn build(self) -> Result<Command, CodeLensError> {
        let title = self.title.ok_or(CodeLensError::MissingField("title"))?;
        let command_id = self
            .command_id
            .ok_or(CodeLensError::MissingField("command_id"))?;
        Ok(Command {
            title,
            command_id,
            tooltip: self.tooltip.unwrap_or_default(),
            arguments: self.arguments,
        })
    }
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

impl fmt::Display for CodeLens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.is_resolved() {
            "resolved"
        } else {
            "unresolved"
        };
        write!(
            f,
            "CodeLens({}:{}-{}:{}, {status})",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
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

    /// Create a code lens with range validation.
    pub fn try_new(
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> Result<Self, CodeLensError> {
        if start_line > end_line || (start_line == end_line && start_col > end_col) {
            return Err(CodeLensError::InvalidRange {
                start_line,
                start_col,
                end_line,
                end_col,
            });
        }
        Ok(Self::new(start_line, start_col, end_line, end_col))
    }

    /// Returns `true` if a command has been attached.
    pub fn is_resolved(&self) -> bool {
        self.command.is_some()
    }

    /// Attach a command to this lens, making it resolved.
    pub fn with_command(mut self, command: Command) -> Self {
        self.command = Some(command);
        self
    }

    /// Attach provider-specific data to this lens.
    pub fn with_data(mut self, data: impl Into<String>) -> Self {
        self.data = data.into();
        self
    }

    /// Returns `true` if this lens spans a single line.
    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }

    /// Number of lines this lens spans (inclusive).
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Returns `true` if the given line falls within this lens's range.
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
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

impl fmt::Debug for CodeLensService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodeLensService")
            .field("provider_count", &self.providers.len())
            .finish()
    }
}

impl CodeLensService {
    /// Collect and immediately resolve all lenses for the given URI.
    pub fn provide_and_resolve(&self, uri: &str) -> Vec<CodeLens> {
        let lenses = self.provide_code_lenses(uri);
        self.resolve_all(lenses)
    }

    /// Resolve lenses, returning an error for any that remain unresolved.
    pub fn resolve_all_strict(&self, lenses: Vec<CodeLens>) -> Result<Vec<CodeLens>, CodeLensError> {
        let resolved = self.resolve_all(lenses);
        for lens in &resolved {
            if !lens.is_resolved() {
                return Err(CodeLensError::UnresolvedLens {
                    data: lens.data.clone(),
                });
            }
        }
        Ok(resolved)
    }

    /// Return only the lenses that overlap the given line.
    pub fn lenses_at_line(&self, uri: &str, line: u32) -> Vec<CodeLens> {
        self.provide_code_lenses(uri)
            .into_iter()
            .filter(|l| l.contains_line(line))
            .collect()
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

    // -- Additional tests ---------------------------------------------------

    #[test]
    fn try_new_validates_range() {
        assert!(CodeLens::try_new(0, 0, 5, 10).is_ok());
        assert!(CodeLens::try_new(5, 0, 5, 10).is_ok());
        assert!(CodeLens::try_new(5, 10, 5, 10).is_ok());

        let err = CodeLens::try_new(5, 11, 5, 10).unwrap_err();
        assert_eq!(
            err,
            CodeLensError::InvalidRange {
                start_line: 5,
                start_col: 11,
                end_line: 5,
                end_col: 10,
            }
        );

        let err2 = CodeLens::try_new(10, 0, 5, 0).unwrap_err();
        assert!(matches!(err2, CodeLensError::InvalidRange { .. }));
    }

    #[test]
    fn code_lens_display() {
        let lens = CodeLens::new(1, 0, 3, 20);
        assert_eq!(format!("{lens}"), "CodeLens(1:0-3:20, unresolved)");

        let resolved = lens.with_command(Command {
            title: "t".into(),
            command_id: "c".into(),
            tooltip: String::new(),
            arguments: vec![],
        });
        assert!(format!("{resolved}").contains("resolved"));
    }

    #[test]
    fn command_display() {
        let cmd = Command {
            title: "Run".into(),
            command_id: "test.run".into(),
            tooltip: String::new(),
            arguments: vec![],
        };
        assert_eq!(format!("{cmd}"), "[test.run] Run");
    }

    #[test]
    fn command_builder_success() {
        let cmd = CommandBuilder::new()
            .title("5 references")
            .command_id("editor.showReferences")
            .tooltip("Show all references")
            .argument("file.rs")
            .argument("line:10")
            .build()
            .unwrap();

        assert_eq!(cmd.title, "5 references");
        assert_eq!(cmd.command_id, "editor.showReferences");
        assert_eq!(cmd.tooltip, "Show all references");
        assert_eq!(cmd.arguments, vec!["file.rs", "line:10"]);
    }

    #[test]
    fn command_builder_missing_title() {
        let result = CommandBuilder::new()
            .command_id("test.run")
            .build();
        assert_eq!(result.unwrap_err(), CodeLensError::MissingField("title"));
    }

    #[test]
    fn command_builder_missing_command_id() {
        let result = CommandBuilder::new()
            .title("Run")
            .build();
        assert_eq!(
            result.unwrap_err(),
            CodeLensError::MissingField("command_id")
        );
    }

    #[test]
    fn code_lens_with_data_and_command() {
        let lens = CodeLens::new(0, 0, 0, 5)
            .with_data("my_provider")
            .with_command(Command {
                title: "Go".into(),
                command_id: "go".into(),
                tooltip: String::new(),
                arguments: vec![],
            });
        assert!(lens.is_resolved());
        assert_eq!(lens.data, "my_provider");
    }

    #[test]
    fn single_line_and_line_span() {
        let single = CodeLens::new(3, 0, 3, 10);
        assert!(single.is_single_line());
        assert_eq!(single.line_span(), 1);

        let multi = CodeLens::new(3, 0, 7, 10);
        assert!(!multi.is_single_line());
        assert_eq!(multi.line_span(), 5);
    }

    #[test]
    fn contains_line() {
        let lens = CodeLens::new(5, 0, 10, 0);
        assert!(!lens.contains_line(4));
        assert!(lens.contains_line(5));
        assert!(lens.contains_line(7));
        assert!(lens.contains_line(10));
        assert!(!lens.contains_line(11));
    }

    #[test]
    fn lenses_at_line_filters_correctly() {
        let mut service = CodeLensService::new();
        service.register(ReferenceCountProvider);

        let at_0 = service.lenses_at_line("main.rs", 0);
        assert_eq!(at_0.len(), 1);
        assert_eq!(at_0[0].start_line, 0);

        let at_5 = service.lenses_at_line("main.rs", 5);
        assert_eq!(at_5.len(), 1);
        assert_eq!(at_5[0].start_line, 5);

        let at_99 = service.lenses_at_line("main.rs", 99);
        assert!(at_99.is_empty());
    }

    #[test]
    fn provide_and_resolve_convenience() {
        let mut service = CodeLensService::new();
        service.register(ReferenceCountProvider);

        let lenses = service.provide_and_resolve("main.rs");
        assert_eq!(lenses.len(), 2);
        assert!(lenses.iter().all(|l| l.is_resolved()));
    }

    #[test]
    fn resolve_all_strict_returns_error_for_unresolved() {
        struct NoopProvider;
        impl CodeLensProvider for NoopProvider {
            fn provide_code_lenses(&self, _uri: &str) -> Vec<CodeLens> {
                vec![CodeLens::new(0, 0, 0, 1).with_data("noop")]
            }
        }

        let mut service = CodeLensService::new();
        service.register(NoopProvider);

        let lenses = service.provide_code_lenses("file.rs");
        let err = service.resolve_all_strict(lenses).unwrap_err();
        assert_eq!(
            err,
            CodeLensError::UnresolvedLens {
                data: "noop".into()
            }
        );
    }

    #[test]
    fn service_debug_impl() {
        let service = CodeLensService::new();
        let dbg = format!("{service:?}");
        assert!(dbg.contains("CodeLensService"));
        assert!(dbg.contains("provider_count"));
    }

    #[test]
    fn error_display_messages() {
        let e1 = CodeLensError::InvalidRange {
            start_line: 5,
            start_col: 3,
            end_line: 2,
            end_col: 1,
        };
        assert!(format!("{e1}").contains("invalid range"));

        let e2 = CodeLensError::MissingField("title");
        assert!(format!("{e2}").contains("title"));

        let e3 = CodeLensError::UnresolvedLens {
            data: "xyz".into(),
        };
        assert!(format!("{e3}").contains("xyz"));
    }
}
