//! Inlay type and parameter annotations for inline editor hints.

/// The kind of inlay hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintKind {
    /// A type annotation hint (e.g. `: i32`).
    Type,
    /// A parameter name hint (e.g. `name:`).
    Parameter,
    /// Any other hint.
    Other,
}

/// A single labeled segment of an inlay hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintLabelPart {
    pub value: String,
    pub tooltip: Option<String>,
    pub command: Option<String>,
}

/// An inlay hint displayed inline in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    pub position_line: u32,
    pub position_col: u32,
    pub label: Vec<InlayHintLabelPart>,
    pub kind: InlayHintKind,
    pub padding_left: bool,
    pub padding_right: bool,
    pub tooltip: Option<String>,
}

impl InlayHint {
    /// Create a simple hint with a single label part and no tooltip or command.
    pub fn simple(
        position_line: u32,
        position_col: u32,
        text: impl Into<String>,
        kind: InlayHintKind,
    ) -> Self {
        Self {
            position_line,
            position_col,
            label: vec![InlayHintLabelPart {
                value: text.into(),
                tooltip: None,
                command: None,
            }],
            kind,
            padding_left: false,
            padding_right: false,
            tooltip: None,
        }
    }
}

/// Trait for types that can provide inlay hints for a document region.
pub trait InlayHintsProvider {
    /// Return inlay hints for the given URI within the line range `[start_line, end_line]`.
    fn provide_inlay_hints(
        &self,
        uri: &str,
        start_line: u32,
        end_line: u32,
    ) -> Vec<InlayHint>;
}

impl std::fmt::Display for InlayHintKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InlayHintKind::Type => write!(f, "Type"),
            InlayHintKind::Parameter => write!(f, "Parameter"),
            InlayHintKind::Other => write!(f, "Other"),
        }
    }
}

impl std::fmt::Display for InlayHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for part in &self.label {
            write!(f, "{}", part.value)?;
        }
        Ok(())
    }
}

/// Errors that can occur when constructing or resolving inlay hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlayHintError {
    /// The specified position is invalid (e.g. out of document bounds).
    InvalidPosition { line: u32, col: u32 },
    /// The hint label is empty; at least one label part is required.
    EmptyLabel,
    /// No provider was found with the given name.
    ProviderNotFound(String),
}

impl std::fmt::Display for InlayHintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InlayHintError::InvalidPosition { line, col } => {
                write!(f, "invalid position: line {line}, col {col}")
            }
            InlayHintError::EmptyLabel => write!(f, "hint label must not be empty"),
            InlayHintError::ProviderNotFound(name) => {
                write!(f, "provider not found: {name}")
            }
        }
    }
}

impl std::error::Error for InlayHintError {}

/// Builder for constructing [`InlayHint`] instances step-by-step.
#[derive(Debug, Clone)]
pub struct InlayHintBuilder {
    position_line: Option<u32>,
    position_col: Option<u32>,
    label: Vec<InlayHintLabelPart>,
    kind: InlayHintKind,
    padding_left: bool,
    padding_right: bool,
    tooltip: Option<String>,
}

impl InlayHintBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self {
            position_line: None,
            position_col: None,
            label: Vec::new(),
            kind: InlayHintKind::Other,
            padding_left: false,
            padding_right: false,
            tooltip: None,
        }
    }

    /// Set the position (line and column) of the hint.
    pub fn position(mut self, line: u32, col: u32) -> Self {
        self.position_line = Some(line);
        self.position_col = Some(col);
        self
    }

    /// Append a label part to the hint.
    pub fn add_label_part(mut self, value: impl Into<String>) -> Self {
        self.label.push(InlayHintLabelPart {
            value: value.into(),
            tooltip: None,
            command: None,
        });
        self
    }

    /// Set the kind of hint.
    pub fn kind(mut self, kind: InlayHintKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set left and right padding.
    pub fn padding(mut self, left: bool, right: bool) -> Self {
        self.padding_left = left;
        self.padding_right = right;
        self
    }

    /// Set the tooltip for the hint.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Build the [`InlayHint`], returning an error if required fields are missing.
    pub fn build(self) -> Result<InlayHint, InlayHintError> {
        let line = self.position_line.ok_or(InlayHintError::InvalidPosition {
            line: u32::MAX,
            col: u32::MAX,
        })?;
        let col = self.position_col.ok_or(InlayHintError::InvalidPosition {
            line,
            col: u32::MAX,
        })?;
        if self.label.is_empty() {
            return Err(InlayHintError::EmptyLabel);
        }
        Ok(InlayHint {
            position_line: line,
            position_col: col,
            label: self.label,
            kind: self.kind,
            padding_left: self.padding_left,
            padding_right: self.padding_right,
            tooltip: self.tooltip,
        })
    }
}

impl Default for InlayHintBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Data attached to a hint for lazy resolution (e.g. deferred tooltip or command).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintResolveData {
    /// An identifier the server can use to look up additional information.
    pub resolve_id: String,
    /// Additional tooltip text loaded on demand.
    pub tooltip: Option<String>,
    /// Command identifier to execute when the hint is clicked.
    pub command_id: Option<String>,
    /// Human-readable command title.
    pub command_title: Option<String>,
}

impl InlayHintResolveData {
    /// Create resolve data with only an identifier.
    pub fn new(resolve_id: impl Into<String>) -> Self {
        Self {
            resolve_id: resolve_id.into(),
            tooltip: None,
            command_id: None,
            command_title: None,
        }
    }

    /// Attach a lazily-resolved tooltip.
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Attach a command.
    pub fn with_command(mut self, id: impl Into<String>, title: impl Into<String>) -> Self {
        self.command_id = Some(id.into());
        self.command_title = Some(title.into());
        self
    }
}

impl InlayHint {
    /// Merge adjacent hints that share the same line into a single hint.
    ///
    /// Hints are considered adjacent when they are on the same line.  The
    /// merged hint keeps the position of the first hint in the group, and
    /// all label parts are concatenated in order.  The kind is taken from
    /// the first hint.
    pub fn merge_adjacent(mut hints: Vec<InlayHint>) -> Vec<InlayHint> {
        if hints.len() <= 1 {
            return hints;
        }
        hints.sort_by(|a, b| {
            a.position_line
                .cmp(&b.position_line)
                .then(a.position_col.cmp(&b.position_col))
        });

        let mut merged: Vec<InlayHint> = Vec::new();
        for hint in hints {
            if let Some(last) = merged.last_mut() {
                if last.position_line == hint.position_line {
                    last.label.extend(hint.label);
                    continue;
                }
            }
            merged.push(hint);
        }
        merged
    }
}

/// Configuration for inlay hints display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintsConfig {
    pub enabled: bool,
    pub font_size: Option<u32>,
    pub font_family: Option<String>,
    /// Maximum display length (in characters) for a single hint label.
    pub max_length: Option<u32>,
}

impl Default for InlayHintsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            font_size: None,
            font_family: None,
            max_length: None,
        }
    }
}

/// A registry that stores multiple named [`InlayHintsProvider`] instances.
///
/// Querying the registry collects hints from every registered provider and
/// returns them sorted by position.
pub struct InlayHintsRegistry {
    providers: Vec<(String, Box<dyn InlayHintsProvider>)>,
}

impl InlayHintsRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider under the given name.
    pub fn register(&mut self, name: impl Into<String>, provider: Box<dyn InlayHintsProvider>) {
        self.providers.push((name.into(), provider));
    }

    /// Remove a provider by name. Returns `true` if it was found.
    pub fn unregister(&mut self, name: &str) -> bool {
        let before = self.providers.len();
        self.providers.retain(|(n, _)| n != name);
        self.providers.len() < before
    }

    /// Query all registered providers for the given range, merge and sort.
    pub fn provide_all(
        &self,
        uri: &str,
        start_line: u32,
        end_line: u32,
    ) -> Vec<InlayHint> {
        let mut all: Vec<InlayHint> = self
            .providers
            .iter()
            .flat_map(|(_, p)| p.provide_inlay_hints(uri, start_line, end_line))
            .collect();
        all.sort_by(|a, b| {
            a.position_line
                .cmp(&b.position_line)
                .then(a.position_col.cmp(&b.position_col))
        });
        all
    }

    /// Query a single provider by name.
    pub fn provide_by_name(
        &self,
        name: &str,
        uri: &str,
        start_line: u32,
        end_line: u32,
    ) -> Result<Vec<InlayHint>, InlayHintError> {
        self.providers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, p)| p.provide_inlay_hints(uri, start_line, end_line))
            .ok_or_else(|| InlayHintError::ProviderNotFound(name.to_string()))
    }

    /// Return the number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Return whether the registry has no providers.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl Default for InlayHintsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_hint_construction() {
        let hint = InlayHint::simple(10, 5, ": i32", InlayHintKind::Type);
        assert_eq!(hint.position_line, 10);
        assert_eq!(hint.position_col, 5);
        assert_eq!(hint.kind, InlayHintKind::Type);
        assert_eq!(hint.label.len(), 1);
        assert_eq!(hint.label[0].value, ": i32");
        assert!(!hint.padding_left);
        assert!(!hint.padding_right);
        assert!(hint.tooltip.is_none());
    }

    #[test]
    fn provider_returns_hints_in_range() {
        struct TestProvider;

        impl InlayHintsProvider for TestProvider {
            fn provide_inlay_hints(
                &self,
                _uri: &str,
                start_line: u32,
                end_line: u32,
            ) -> Vec<InlayHint> {
                (start_line..=end_line)
                    .map(|line| InlayHint::simple(line, 0, "hint", InlayHintKind::Other))
                    .collect()
            }
        }

        let provider = TestProvider;
        let hints = provider.provide_inlay_hints("file:///test.rs", 3, 5);
        assert_eq!(hints.len(), 3);
        assert_eq!(hints[0].position_line, 3);
        assert_eq!(hints[2].position_line, 5);
    }

    #[test]
    fn default_config_is_enabled() {
        let config = InlayHintsConfig::default();
        assert!(config.enabled);
        assert!(config.font_size.is_none());
        assert!(config.font_family.is_none());
        assert!(config.max_length.is_none());
    }

    #[test]
    fn builder_valid_hint() {
        let hint = InlayHintBuilder::new()
            .position(1, 2)
            .add_label_part(": u64")
            .kind(InlayHintKind::Type)
            .padding(true, false)
            .tooltip("unsigned 64-bit integer")
            .build()
            .expect("should build successfully");

        assert_eq!(hint.position_line, 1);
        assert_eq!(hint.position_col, 2);
        assert_eq!(hint.kind, InlayHintKind::Type);
        assert!(hint.padding_left);
        assert!(!hint.padding_right);
        assert_eq!(hint.tooltip.as_deref(), Some("unsigned 64-bit integer"));
        assert_eq!(hint.label[0].value, ": u64");
    }

    #[test]
    fn builder_missing_position() {
        let result = InlayHintBuilder::new()
            .add_label_part("text")
            .build();
        assert!(matches!(result, Err(InlayHintError::InvalidPosition { .. })));
    }

    #[test]
    fn builder_empty_label() {
        let result = InlayHintBuilder::new()
            .position(0, 0)
            .build();
        assert_eq!(result, Err(InlayHintError::EmptyLabel));
    }

    #[test]
    fn display_inlay_hint_kind() {
        assert_eq!(format!("{}", InlayHintKind::Type), "Type");
        assert_eq!(format!("{}", InlayHintKind::Parameter), "Parameter");
        assert_eq!(format!("{}", InlayHintKind::Other), "Other");
    }

    #[test]
    fn display_inlay_hint_concatenates_labels() {
        let hint = InlayHintBuilder::new()
            .position(0, 0)
            .add_label_part("name")
            .add_label_part(": ")
            .add_label_part("String")
            .kind(InlayHintKind::Type)
            .build()
            .unwrap();
        assert_eq!(format!("{hint}"), "name: String");
    }

    #[test]
    fn error_display_messages() {
        let e1 = InlayHintError::InvalidPosition { line: 5, col: 10 };
        assert_eq!(format!("{e1}"), "invalid position: line 5, col 10");

        let e2 = InlayHintError::EmptyLabel;
        assert_eq!(format!("{e2}"), "hint label must not be empty");

        let e3 = InlayHintError::ProviderNotFound("foo".into());
        assert_eq!(format!("{e3}"), "provider not found: foo");
    }

    #[test]
    fn registry_multiple_providers() {
        struct TypeHinter;
        impl InlayHintsProvider for TypeHinter {
            fn provide_inlay_hints(&self, _uri: &str, _s: u32, _e: u32) -> Vec<InlayHint> {
                vec![InlayHint::simple(2, 10, ": i32", InlayHintKind::Type)]
            }
        }

        struct ParamHinter;
        impl InlayHintsProvider for ParamHinter {
            fn provide_inlay_hints(&self, _uri: &str, _s: u32, _e: u32) -> Vec<InlayHint> {
                vec![InlayHint::simple(1, 5, "name:", InlayHintKind::Parameter)]
            }
        }

        let mut registry = InlayHintsRegistry::new();
        assert!(registry.is_empty());
        registry.register("types", Box::new(TypeHinter));
        registry.register("params", Box::new(ParamHinter));
        assert_eq!(registry.len(), 2);

        let hints = registry.provide_all("file:///test.rs", 0, 10);
        assert_eq!(hints.len(), 2);
        // Should be sorted by position: line 1 before line 2.
        assert_eq!(hints[0].position_line, 1);
        assert_eq!(hints[1].position_line, 2);
    }

    #[test]
    fn registry_provide_by_name_not_found() {
        let registry = InlayHintsRegistry::new();
        let result = registry.provide_by_name("missing", "file:///x", 0, 10);
        assert_eq!(
            result,
            Err(InlayHintError::ProviderNotFound("missing".into()))
        );
    }

    #[test]
    fn registry_unregister() {
        struct Dummy;
        impl InlayHintsProvider for Dummy {
            fn provide_inlay_hints(&self, _: &str, _: u32, _: u32) -> Vec<InlayHint> {
                vec![]
            }
        }

        let mut registry = InlayHintsRegistry::new();
        registry.register("dummy", Box::new(Dummy));
        assert_eq!(registry.len(), 1);
        assert!(registry.unregister("dummy"));
        assert!(registry.is_empty());
        assert!(!registry.unregister("dummy"));
    }

    #[test]
    fn merge_adjacent_same_line() {
        let hints = vec![
            InlayHint::simple(5, 10, ": i32", InlayHintKind::Type),
            InlayHint::simple(5, 20, ": u8", InlayHintKind::Type),
            InlayHint::simple(7, 3, "x:", InlayHintKind::Parameter),
        ];
        let merged = InlayHint::merge_adjacent(hints);
        assert_eq!(merged.len(), 2);
        // First merged hint should have two label parts from line 5.
        assert_eq!(merged[0].position_line, 5);
        assert_eq!(merged[0].label.len(), 2);
        assert_eq!(merged[0].label[0].value, ": i32");
        assert_eq!(merged[0].label[1].value, ": u8");
        // Second hint is on line 7.
        assert_eq!(merged[1].position_line, 7);
        assert_eq!(merged[1].label.len(), 1);
    }

    #[test]
    fn merge_adjacent_empty_and_single() {
        assert!(InlayHint::merge_adjacent(vec![]).is_empty());
        let single = vec![InlayHint::simple(0, 0, "x", InlayHintKind::Other)];
        let result = InlayHint::merge_adjacent(single);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn resolve_data_construction() {
        let data = InlayHintResolveData::new("hint-42")
            .with_tooltip("Full type: std::string::String")
            .with_command("editor.action.showType", "Show Full Type");

        assert_eq!(data.resolve_id, "hint-42");
        assert_eq!(
            data.tooltip.as_deref(),
            Some("Full type: std::string::String")
        );
        assert_eq!(data.command_id.as_deref(), Some("editor.action.showType"));
        assert_eq!(data.command_title.as_deref(), Some("Show Full Type"));
    }
}
