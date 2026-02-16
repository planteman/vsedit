//! Function signature help.

/// Information about a single parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterInformation {
    pub label: String,
    pub documentation: Option<String>,
}

/// Information about a function signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInformation {
    pub label: String,
    pub documentation: Option<String>,
    pub parameters: Vec<ParameterInformation>,
    pub active_parameter: Option<u32>,
}

/// The result of a signature help request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    pub active_signature: u32,
    pub active_parameter: u32,
}

impl SignatureHelp {
    /// Returns the currently active signature, if any.
    pub fn active_signature_info(&self) -> Option<&SignatureInformation> {
        self.signatures.get(self.active_signature as usize)
    }

    /// Returns the currently active parameter of the active signature, if any.
    pub fn active_param_info(&self) -> Option<&ParameterInformation> {
        let sig = self.active_signature_info()?;
        let idx = sig.active_parameter.unwrap_or(self.active_parameter);
        sig.parameters.get(idx as usize)
    }
}

/// How signature help was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureHelpTriggerKind {
    Invoke,
    TriggerCharacter,
    ContentChange,
}

/// Context for a signature help request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelpContext {
    pub trigger_kind: SignatureHelpTriggerKind,
    pub trigger_character: Option<char>,
    pub is_retrigger: bool,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during signature help operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureHelpError {
    /// No signatures are available.
    NoSignatures,
    /// The requested index is out of range.
    InvalidIndex,
    /// The underlying provider failed.
    ProviderFailed(String),
}

impl std::fmt::Display for SignatureHelpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSignatures => write!(f, "no signatures available"),
            Self::InvalidIndex => write!(f, "index out of range"),
            Self::ProviderFailed(msg) => write!(f, "provider failed: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

impl std::fmt::Display for SignatureInformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.label)?;
        for (i, p) in self.parameters.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", p.label)?;
        }
        write!(f, ")")
    }
}

impl std::fmt::Display for SignatureHelpTriggerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invoke => write!(f, "Invoke"),
            Self::TriggerCharacter => write!(f, "TriggerCharacter"),
            Self::ContentChange => write!(f, "ContentChange"),
        }
    }
}

// ---------------------------------------------------------------------------
// Extra helpers on SignatureInformation
// ---------------------------------------------------------------------------

impl SignatureInformation {
    /// Returns the number of parameters in this signature.
    pub fn parameter_count(&self) -> usize {
        self.parameters.len()
    }

    /// Returns `true` if this signature has documentation.
    pub fn has_documentation(&self) -> bool {
        self.documentation.is_some()
    }
}

// ---------------------------------------------------------------------------
// Navigation helpers on SignatureHelp
// ---------------------------------------------------------------------------

impl SignatureHelp {
    /// Move to the next signature, wrapping around if `cycle` is true.
    pub fn next_signature(&mut self, cycle: bool) {
        if self.signatures.is_empty() {
            return;
        }
        let len = self.signatures.len() as u32;
        if self.active_signature + 1 < len {
            self.active_signature += 1;
        } else if cycle {
            self.active_signature = 0;
        }
    }

    /// Move to the previous signature, wrapping around if `cycle` is true.
    pub fn prev_signature(&mut self, cycle: bool) {
        if self.signatures.is_empty() {
            return;
        }
        if self.active_signature > 0 {
            self.active_signature -= 1;
        } else if cycle {
            self.active_signature = self.signatures.len() as u32 - 1;
        }
    }

    /// Move to the next parameter of the active signature, wrapping if `cycle`.
    pub fn next_parameter(&mut self, cycle: bool) {
        if let Some(sig) = self.signatures.get(self.active_signature as usize) {
            let len = sig.parameters.len() as u32;
            if len == 0 {
                return;
            }
            if self.active_parameter + 1 < len {
                self.active_parameter += 1;
            } else if cycle {
                self.active_parameter = 0;
            }
        }
    }

    /// Move to the previous parameter of the active signature, wrapping if `cycle`.
    pub fn prev_parameter(&mut self, cycle: bool) {
        if let Some(sig) = self.signatures.get(self.active_signature as usize) {
            let len = sig.parameters.len() as u32;
            if len == 0 {
                return;
            }
            if self.active_parameter > 0 {
                self.active_parameter -= 1;
            } else if cycle {
                self.active_parameter = len - 1;
            }
        }
    }

    /// Convenience: returns the label of the active signature, if any.
    pub fn active_signature_label(&self) -> Option<&str> {
        self.active_signature_info().map(|s| s.label.as_str())
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for signature help behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelpConfig {
    /// Whether signature help is enabled.
    pub enabled: bool,
    /// Characters that trigger signature help.
    pub trigger_characters: Vec<char>,
    /// Characters that re-trigger signature help.
    pub retrigger_characters: Vec<char>,
    /// Whether navigation should cycle around the list.
    pub cycle: bool,
}

impl Default for SignatureHelpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger_characters: vec!['(', ','],
            retrigger_characters: vec![','],
            cycle: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Provider trait & registry
// ---------------------------------------------------------------------------

/// Provides signature help for function calls.
pub trait SignatureHelpProvider {
    fn provide_signature_help(
        &self,
        uri: &str,
        line: u32,
        col: u32,
        context: &SignatureHelpContext,
    ) -> Option<SignatureHelp>;
}

/// A registry that stores multiple providers and queries them in order.
///
/// The first provider that returns `Some` wins.
pub struct SignatureHelpRegistry {
    providers: Vec<Box<dyn SignatureHelpProvider>>,
}

impl SignatureHelpRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Box<dyn SignatureHelpProvider>) {
        self.providers.push(provider);
    }

    /// Query all providers in registration order; return the first `Some`.
    pub fn provide_signature_help(
        &self,
        uri: &str,
        line: u32,
        col: u32,
        context: &SignatureHelpContext,
    ) -> Option<SignatureHelp> {
        for provider in &self.providers {
            if let Some(help) = provider.provide_signature_help(uri, line, col, context) {
                return Some(help);
            }
        }
        None
    }

    /// Returns the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

impl Default for SignatureHelpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_signature() -> SignatureInformation {
        SignatureInformation {
            label: "fn foo(x: i32, y: &str)".into(),
            documentation: Some("Does foo things.".into()),
            parameters: vec![
                ParameterInformation {
                    label: "x: i32".into(),
                    documentation: Some("The x value.".into()),
                },
                ParameterInformation {
                    label: "y: &str".into(),
                    documentation: None,
                },
            ],
            active_parameter: None,
        }
    }

    #[test]
    fn active_signature_info() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 0,
        };
        let sig = help.active_signature_info().unwrap();
        assert_eq!(sig.label, "fn foo(x: i32, y: &str)");
    }

    #[test]
    fn active_param_info() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 1,
        };
        let param = help.active_param_info().unwrap();
        assert_eq!(param.label, "y: &str");
    }

    #[test]
    fn out_of_bounds_returns_none() {
        let help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        assert!(help.active_signature_info().is_none());
        assert!(help.active_param_info().is_none());
    }

    struct DummyProvider;

    impl SignatureHelpProvider for DummyProvider {
        fn provide_signature_help(
            &self,
            _uri: &str,
            _line: u32,
            _col: u32,
            context: &SignatureHelpContext,
        ) -> Option<SignatureHelp> {
            if context.trigger_kind == SignatureHelpTriggerKind::Invoke {
                Some(SignatureHelp {
                    signatures: vec![sample_signature()],
                    active_signature: 0,
                    active_parameter: 0,
                })
            } else {
                None
            }
        }
    }

    #[test]
    fn provider_returns_help_on_invoke() {
        let provider = DummyProvider;
        let ctx = SignatureHelpContext {
            trigger_kind: SignatureHelpTriggerKind::Invoke,
            trigger_character: None,
            is_retrigger: false,
        };
        let help = provider
            .provide_signature_help("file:///main.rs", 5, 10, &ctx)
            .unwrap();
        assert_eq!(help.signatures.len(), 1);
    }

    // -----------------------------------------------------------------------
    // New tests
    // -----------------------------------------------------------------------

    fn two_signature_help() -> SignatureHelp {
        SignatureHelp {
            signatures: vec![
                sample_signature(),
                SignatureInformation {
                    label: "fn bar(a: bool)".into(),
                    documentation: None,
                    parameters: vec![ParameterInformation {
                        label: "a: bool".into(),
                        documentation: None,
                    }],
                    active_parameter: None,
                },
            ],
            active_signature: 0,
            active_parameter: 0,
        }
    }

    #[test]
    fn next_signature_cycles() {
        let mut help = two_signature_help();
        help.next_signature(true);
        assert_eq!(help.active_signature, 1);
        help.next_signature(true);
        assert_eq!(help.active_signature, 0); // wrapped
    }

    #[test]
    fn next_signature_no_cycle_clamps() {
        let mut help = two_signature_help();
        help.next_signature(false);
        assert_eq!(help.active_signature, 1);
        help.next_signature(false);
        assert_eq!(help.active_signature, 1); // clamped
    }

    #[test]
    fn prev_signature_cycles() {
        let mut help = two_signature_help();
        assert_eq!(help.active_signature, 0);
        help.prev_signature(true);
        assert_eq!(help.active_signature, 1); // wrapped
        help.prev_signature(true);
        assert_eq!(help.active_signature, 0);
    }

    #[test]
    fn prev_signature_no_cycle_clamps() {
        let mut help = two_signature_help();
        help.prev_signature(false);
        assert_eq!(help.active_signature, 0); // clamped at 0
    }

    #[test]
    fn next_prev_parameter() {
        let mut help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 0,
        };
        help.next_parameter(false);
        assert_eq!(help.active_parameter, 1);
        help.next_parameter(false);
        assert_eq!(help.active_parameter, 1); // clamped
        help.next_parameter(true);
        assert_eq!(help.active_parameter, 0); // cycled

        help.prev_parameter(false);
        assert_eq!(help.active_parameter, 0); // clamped
        help.prev_parameter(true);
        assert_eq!(help.active_parameter, 1); // cycled
    }

    #[test]
    fn active_signature_label_returns_label() {
        let help = two_signature_help();
        assert_eq!(
            help.active_signature_label(),
            Some("fn foo(x: i32, y: &str)")
        );
    }

    #[test]
    fn display_signature_information() {
        let sig = sample_signature();
        let text = format!("{sig}");
        assert_eq!(text, "fn foo(x: i32, y: &str)(x: i32, y: &str)");
    }

    #[test]
    fn display_trigger_kind() {
        assert_eq!(format!("{}", SignatureHelpTriggerKind::Invoke), "Invoke");
        assert_eq!(
            format!("{}", SignatureHelpTriggerKind::TriggerCharacter),
            "TriggerCharacter"
        );
        assert_eq!(
            format!("{}", SignatureHelpTriggerKind::ContentChange),
            "ContentChange"
        );
    }

    #[test]
    fn parameter_count_and_has_documentation() {
        let sig = sample_signature();
        assert_eq!(sig.parameter_count(), 2);
        assert!(sig.has_documentation());

        let bare = SignatureInformation {
            label: "fn bare()".into(),
            documentation: None,
            parameters: vec![],
            active_parameter: None,
        };
        assert_eq!(bare.parameter_count(), 0);
        assert!(!bare.has_documentation());
    }

    #[test]
    fn config_defaults() {
        let cfg = SignatureHelpConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.trigger_characters, vec!['(', ',']);
        assert_eq!(cfg.retrigger_characters, vec![',']);
        assert!(cfg.cycle);
    }

    #[test]
    fn registry_queries_in_order() {
        struct NullProvider;
        impl SignatureHelpProvider for NullProvider {
            fn provide_signature_help(
                &self,
                _uri: &str,
                _line: u32,
                _col: u32,
                _ctx: &SignatureHelpContext,
            ) -> Option<SignatureHelp> {
                None
            }
        }

        let mut registry = SignatureHelpRegistry::new();
        registry.register(Box::new(NullProvider));
        registry.register(Box::new(DummyProvider));
        assert_eq!(registry.provider_count(), 2);

        let ctx = SignatureHelpContext {
            trigger_kind: SignatureHelpTriggerKind::Invoke,
            trigger_character: None,
            is_retrigger: false,
        };
        // NullProvider returns None, DummyProvider returns Some
        let help = registry
            .provide_signature_help("file:///main.rs", 1, 1, &ctx)
            .unwrap();
        assert_eq!(help.signatures.len(), 1);
    }

    #[test]
    fn registry_returns_none_when_all_fail() {
        struct NullProvider;
        impl SignatureHelpProvider for NullProvider {
            fn provide_signature_help(
                &self,
                _uri: &str,
                _line: u32,
                _col: u32,
                _ctx: &SignatureHelpContext,
            ) -> Option<SignatureHelp> {
                None
            }
        }

        let mut registry = SignatureHelpRegistry::new();
        registry.register(Box::new(NullProvider));

        let ctx = SignatureHelpContext {
            trigger_kind: SignatureHelpTriggerKind::ContentChange,
            trigger_character: None,
            is_retrigger: false,
        };
        assert!(registry
            .provide_signature_help("file:///x.rs", 0, 0, &ctx)
            .is_none());
    }

    #[test]
    fn error_display() {
        assert_eq!(
            SignatureHelpError::NoSignatures.to_string(),
            "no signatures available"
        );
        assert_eq!(
            SignatureHelpError::InvalidIndex.to_string(),
            "index out of range"
        );
        assert_eq!(
            SignatureHelpError::ProviderFailed("timeout".into()).to_string(),
            "provider failed: timeout"
        );
    }

    #[test]
    fn navigation_on_empty_signatures() {
        let mut help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        help.next_signature(true);
        assert_eq!(help.active_signature, 0);
        help.prev_signature(true);
        assert_eq!(help.active_signature, 0);
        help.next_parameter(true);
        assert_eq!(help.active_parameter, 0);
        help.prev_parameter(true);
        assert_eq!(help.active_parameter, 0);
    }
}
