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

/// Configuration for inlay hints display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintsConfig {
    pub enabled: bool,
    pub font_size: Option<u32>,
    pub font_family: Option<String>,
}

impl Default for InlayHintsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            font_size: None,
            font_family: None,
        }
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
    }
}
