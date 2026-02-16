//! Function signature help.

use std::fmt;
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

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Aggregated statistics about a [`SignatureHelp`] instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamHintStats {
    /// Total number of signatures available.
    pub total_signatures: usize,
    /// Sum of parameters across all signatures.
    pub total_parameters: usize,
    /// Number of signatures whose active parameter is set (i.e. currently
    /// presenting a highlighted hint to the user).
    pub active_hints: usize,
}

/// Compute aggregated statistics for the given [`SignatureHelp`].
pub fn compute_param_hint_stats(help: &SignatureHelp) -> ParamHintStats {
    let total_signatures = help.signatures.len();
    let total_parameters: usize = help.signatures.iter().map(|s| s.parameters.len()).sum();
    let active_hints = help
        .signatures
        .iter()
        .filter(|s| s.active_parameter.is_some())
        .count();
    ParamHintStats {
        total_signatures,
        total_parameters,
        active_hints,
    }
}

// ---------------------------------------------------------------------------
// SignatureHelpWidget — rendering helpers
// ---------------------------------------------------------------------------

/// Render a signature help result to displayable lines.
///
/// The active parameter is wrapped in `[brackets]` for emphasis.
/// Includes overload navigation hint when multiple signatures exist.
pub fn render_signature_help(help: &SignatureHelp, max_width: u16) -> Vec<String> {
    let mut output = Vec::new();

    if help.signatures.is_empty() {
        return output;
    }

    let sig = match help.active_signature_info() {
        Some(s) => s,
        None => return output,
    };

    // Overload indicator
    if help.signatures.len() > 1 {
        output.push(format!(
            "{}/{} overloads (↑/↓ to switch)",
            help.active_signature + 1,
            help.signatures.len()
        ));
    }

    // Build the signature line with active parameter highlighted
    let active_idx = sig.active_parameter.unwrap_or(help.active_parameter) as usize;
    let mut sig_line = String::new();
    sig_line.push_str(&sig.label);
    sig_line.push('(');
    for (i, param) in sig.parameters.iter().enumerate() {
        if i > 0 {
            sig_line.push_str(", ");
        }
        if i == active_idx {
            sig_line.push('[');
            sig_line.push_str(&param.label);
            sig_line.push(']');
        } else {
            sig_line.push_str(&param.label);
        }
    }
    sig_line.push(')');

    // Word-wrap the signature line
    let max_w = max_width as usize;
    if sig_line.len() > max_w && max_w > 0 {
        let mut remaining = sig_line.as_str();
        while remaining.len() > max_w {
            output.push(remaining[..max_w].to_string());
            remaining = &remaining[max_w..];
        }
        if !remaining.is_empty() {
            output.push(remaining.to_string());
        }
    } else {
        output.push(sig_line);
    }

    // Show active parameter documentation if available
    if let Some(param) = sig.parameters.get(active_idx) {
        if let Some(ref doc) = param.documentation {
            output.push(format!("  {}", doc));
        }
    }

    // Show signature documentation
    if let Some(ref doc) = sig.documentation {
        output.push(String::new());
        output.push(doc.clone());
    }

    output
}

/// Check whether a character should trigger signature help.
pub fn should_trigger(ch: char, config: &SignatureHelpConfig) -> bool {
    config.enabled && config.trigger_characters.contains(&ch)
}

/// Check whether a character should re-trigger signature help.
pub fn should_retrigger(ch: char, config: &SignatureHelpConfig) -> bool {
    config.enabled && config.retrigger_characters.contains(&ch)
}

/// Check whether a character should dismiss signature help.
pub fn should_dismiss(ch: char) -> bool {
    ch == ')' || ch == ';'
}

/// Computed layout for the signature help overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureHelpWidget {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl SignatureHelpWidget {
    /// Compute overlay position above the cursor.
    pub fn compute(
        lines: &[String],
        cursor_x: u16,
        cursor_y: u16,
        max_width: u16,
        max_height: u16,
    ) -> Self {
        let content_width = lines
            .iter()
            .map(|l| l.len() as u16)
            .max()
            .unwrap_or(0)
            .min(max_width.saturating_sub(2))
            .max(10);
        let content_height = (lines.len() as u16)
            .min(max_height.saturating_sub(2))
            .max(1);

        let width = content_width + 2;
        let height = content_height + 2;

        let x = if cursor_x + width <= max_width {
            cursor_x
        } else {
            max_width.saturating_sub(width)
        };

        // Prefer showing above cursor
        let y = if cursor_y >= height + 1 {
            cursor_y - height - 1
        } else {
            cursor_y + 1
        };

        Self { x, y, width, height }
    }
}

// ---------------------------------------------------------------------------
// Parameter type extraction
// ---------------------------------------------------------------------------

/// Extract the type portion from a parameter label like "x: i32" → "i32".
pub fn extract_parameter_type(label: &str) -> Option<&str> {
    let colon_pos = label.find(':')?;
    let type_part = label[colon_pos + 1..].trim();
    if type_part.is_empty() { None } else { Some(type_part) }
}

/// Extract the name portion from a parameter label like "x: i32" → "x".
pub fn extract_parameter_name(label: &str) -> &str {
    match label.find(':') {
        Some(pos) => label[..pos].trim(),
        None => label.trim(),
    }
}

// ---------------------------------------------------------------------------
// Overload ranking
// ---------------------------------------------------------------------------

/// Rank an overload based on how many parameters match the provided argument count.
/// Returns a score where higher is better.
pub fn rank_overload(sig: &SignatureInformation, arg_count: usize) -> i32 {
    let param_count = sig.parameters.len();
    if param_count == arg_count {
        100
    } else if arg_count < param_count {
        50 - (param_count as i32 - arg_count as i32)
    } else {
        0
    }
}

/// Sort signatures by relevance to the given argument count.
/// Returns indices sorted from best to worst match.
pub fn rank_overloads(signatures: &[SignatureInformation], arg_count: usize) -> Vec<usize> {
    let mut indexed: Vec<(usize, i32)> = signatures.iter()
        .enumerate()
        .map(|(i, s)| (i, rank_overload(s, arg_count)))
        .collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1));
    indexed.into_iter().map(|(i, _)| i).collect()
}

// ---------------------------------------------------------------------------
// Signature formatting
// ---------------------------------------------------------------------------

/// Format a signature with the active parameter highlighted using brackets.
pub fn format_signature_with_highlight(sig: &SignatureInformation, active_param: u32) -> String {
    let mut result = format!("{}(", sig.label);
    for (i, p) in sig.parameters.iter().enumerate() {
        if i > 0 {
            result.push_str(", ");
        }
        if i as u32 == active_param {
            result.push('[');
            result.push_str(&p.label);
            result.push(']');
        } else {
            result.push_str(&p.label);
        }
    }
    result.push(')');
    result
}

/// Compute the character range of the active parameter within the signature label.
pub fn active_parameter_range(sig: &SignatureInformation, active_param: u32) -> Option<(usize, usize)> {
    let param = sig.parameters.get(active_param as usize)?;
    let label_str = format!("{}", sig);
    let start = label_str.find(&param.label)?;
    Some((start, start + param.label.len()))
}

// ---------------------------------------------------------------------------
// ParameterHintCycle – cycling through overloads
// ---------------------------------------------------------------------------

/// Manages cycling through multiple signature overloads.
#[derive(Debug, Clone)]
pub struct ParameterHintCycle {
    total_signatures: usize,
    current_index: usize,
}

impl ParameterHintCycle {
    /// Create a new cycle with the given number of signatures.
    pub fn new(total_signatures: usize) -> Self {
        Self {
            total_signatures,
            current_index: 0,
        }
    }

    /// Advance to the next overload, wrapping around.
    pub fn next(&mut self) -> usize {
        if self.total_signatures == 0 {
            return 0;
        }
        self.current_index = (self.current_index + 1) % self.total_signatures;
        self.current_index
    }

    /// Go to the previous overload, wrapping around.
    pub fn prev(&mut self) -> usize {
        if self.total_signatures == 0 {
            return 0;
        }
        if self.current_index == 0 {
            self.current_index = self.total_signatures - 1;
        } else {
            self.current_index -= 1;
        }
        self.current_index
    }

    /// Jump to a specific index. Returns `false` if out of range.
    pub fn set_index(&mut self, idx: usize) -> bool {
        if idx < self.total_signatures {
            self.current_index = idx;
            true
        } else {
            false
        }
    }

    /// Current signature index.
    pub fn current(&self) -> usize {
        self.current_index
    }

    /// Total number of signatures.
    pub fn total(&self) -> usize {
        self.total_signatures
    }

    /// Format the cycle indicator, e.g., "2/5".
    pub fn display_indicator(&self) -> String {
        if self.total_signatures == 0 {
            return String::new();
        }
        format!("{}/{}", self.current_index + 1, self.total_signatures)
    }

    /// Apply this cycle to a `SignatureHelp`, updating its `active_signature`.
    pub fn apply_to(&self, help: &mut SignatureHelp) {
        help.active_signature = self.current_index as u32;
    }

    /// Update the total and reset to 0 if the total changed.
    pub fn update_total(&mut self, new_total: usize) {
        if new_total != self.total_signatures {
            self.total_signatures = new_total;
            self.current_index = 0;
        }
    }
}

impl fmt::Display for ParameterHintCycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_indicator())
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

    // -----------------------------------------------------------------------
    // ParamHintStats tests
    // -----------------------------------------------------------------------

    #[test]
    fn stats_empty_signatures() {
        let help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        let stats = compute_param_hint_stats(&help);
        assert_eq!(
            stats,
            ParamHintStats {
                total_signatures: 0,
                total_parameters: 0,
                active_hints: 0,
            }
        );
    }

    #[test]
    fn stats_single_signature_no_active_hint() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 0,
        };
        let stats = compute_param_hint_stats(&help);
        assert_eq!(stats.total_signatures, 1);
        assert_eq!(stats.total_parameters, 2);
        assert_eq!(stats.active_hints, 0);
    }

    #[test]
    fn stats_multiple_signatures_with_active_hints() {
        let help = SignatureHelp {
            signatures: vec![
                SignatureInformation {
                    label: "fn a(x: i32)".into(),
                    documentation: None,
                    parameters: vec![ParameterInformation {
                        label: "x: i32".into(),
                        documentation: None,
                    }],
                    active_parameter: Some(0),
                },
                SignatureInformation {
                    label: "fn b(a: u8, b: u8, c: u8)".into(),
                    documentation: None,
                    parameters: vec![
                        ParameterInformation { label: "a: u8".into(), documentation: None },
                        ParameterInformation { label: "b: u8".into(), documentation: None },
                        ParameterInformation { label: "c: u8".into(), documentation: None },
                    ],
                    active_parameter: None,
                },
                SignatureInformation {
                    label: "fn c()".into(),
                    documentation: None,
                    parameters: vec![],
                    active_parameter: Some(0),
                },
            ],
            active_signature: 0,
            active_parameter: 0,
        };
        let stats = compute_param_hint_stats(&help);
        assert_eq!(stats.total_signatures, 3);
        assert_eq!(stats.total_parameters, 4); // 1 + 3 + 0
        assert_eq!(stats.active_hints, 2); // first and third
    }

    #[test]
    fn stats_all_signatures_active() {
        let make_sig = |n: usize| SignatureInformation {
            label: format!("fn s{n}()"),
            documentation: None,
            parameters: vec![ParameterInformation {
                label: "p".into(),
                documentation: None,
            }],
            active_parameter: Some(0),
        };
        let help = SignatureHelp {
            signatures: vec![make_sig(0), make_sig(1), make_sig(2)],
            active_signature: 0,
            active_parameter: 0,
        };
        let stats = compute_param_hint_stats(&help);
        assert_eq!(stats.total_signatures, 3);
        assert_eq!(stats.total_parameters, 3);
        assert_eq!(stats.active_hints, 3);
    }

    #[test]
    fn stats_two_signature_help_helper() {
        let help = two_signature_help();
        let stats = compute_param_hint_stats(&help);
        assert_eq!(stats.total_signatures, 2);
        assert_eq!(stats.total_parameters, 3); // 2 + 1
        assert_eq!(stats.active_hints, 0);
    }

    // -----------------------------------------------------------------------
    // Rendering & trigger tests
    // -----------------------------------------------------------------------

    #[test]
    fn render_signature_help_basic() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 0,
        };
        let lines = render_signature_help(&help, 80);
        assert!(!lines.is_empty());
        // Active param should be highlighted with brackets
        assert!(lines.iter().any(|l| l.contains("[x: i32]")));
    }

    #[test]
    fn render_signature_help_second_param() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 1,
        };
        let lines = render_signature_help(&help, 80);
        assert!(lines.iter().any(|l| l.contains("[y: &str]")));
    }

    #[test]
    fn render_signature_help_overloads() {
        let help = two_signature_help();
        let lines = render_signature_help(&help, 80);
        assert!(lines.iter().any(|l| l.contains("1/2 overloads")));
    }

    #[test]
    fn render_signature_help_empty() {
        let help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        let lines = render_signature_help(&help, 80);
        assert!(lines.is_empty());
    }

    #[test]
    fn render_signature_help_with_docs() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 0,
        };
        let lines = render_signature_help(&help, 80);
        // sample_signature has documentation for param 0
        assert!(lines.iter().any(|l| l.contains("The x value")));
    }

    #[test]
    fn should_trigger_open_paren() {
        let cfg = SignatureHelpConfig::default();
        assert!(should_trigger('(', &cfg));
        assert!(should_trigger(',', &cfg));
        assert!(!should_trigger(')', &cfg));
        assert!(!should_trigger('a', &cfg));
    }

    #[test]
    fn should_retrigger_comma() {
        let cfg = SignatureHelpConfig::default();
        assert!(should_retrigger(',', &cfg));
        assert!(!should_retrigger('(', &cfg));
    }

    #[test]
    fn should_dismiss_chars() {
        assert!(should_dismiss(')'));
        assert!(should_dismiss(';'));
        assert!(!should_dismiss(','));
        assert!(!should_dismiss('a'));
    }

    #[test]
    fn signature_help_widget_compute() {
        let lines = vec!["fn foo(x: i32, y: &str)".to_string()];
        let widget = SignatureHelpWidget::compute(&lines, 10, 15, 80, 24);
        assert!(widget.width > 0);
        assert!(widget.height > 0);
        assert!(widget.y < 15); // should be above cursor
    }

    #[test]
    fn should_trigger_disabled() {
        let cfg = SignatureHelpConfig {
            enabled: false,
            ..SignatureHelpConfig::default()
        };
        assert!(!should_trigger('(', &cfg));
        assert!(!should_retrigger(',', &cfg));
    }

    #[test]
    fn hint_cycle_next_wraps() {
        let mut cycle = ParameterHintCycle::new(3);
        assert_eq!(cycle.current(), 0);
        assert_eq!(cycle.next(), 1);
        assert_eq!(cycle.next(), 2);
        assert_eq!(cycle.next(), 0); // wraps
    }

    #[test]
    fn hint_cycle_prev_wraps() {
        let mut cycle = ParameterHintCycle::new(3);
        assert_eq!(cycle.prev(), 2); // wraps backward
        assert_eq!(cycle.prev(), 1);
        assert_eq!(cycle.prev(), 0);
    }

    #[test]
    fn hint_cycle_set_index() {
        let mut cycle = ParameterHintCycle::new(5);
        assert!(cycle.set_index(3));
        assert_eq!(cycle.current(), 3);
        assert!(!cycle.set_index(10)); // out of range
        assert_eq!(cycle.current(), 3); // unchanged
    }

    #[test]
    fn hint_cycle_display_indicator() {
        let mut cycle = ParameterHintCycle::new(5);
        assert_eq!(cycle.display_indicator(), "1/5");
        cycle.next();
        assert_eq!(cycle.display_indicator(), "2/5");
    }

    #[test]
    fn hint_cycle_empty() {
        let mut cycle = ParameterHintCycle::new(0);
        assert_eq!(cycle.next(), 0);
        assert_eq!(cycle.prev(), 0);
        assert_eq!(cycle.display_indicator(), "");
    }

    #[test]
    fn hint_cycle_apply_to_help() {
        let mut cycle = ParameterHintCycle::new(3);
        cycle.next();
        cycle.next();
        let mut help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        cycle.apply_to(&mut help);
        assert_eq!(help.active_signature, 2);
    }

    #[test]
    fn hint_cycle_update_total_resets() {
        let mut cycle = ParameterHintCycle::new(5);
        cycle.set_index(3);
        cycle.update_total(10);
        assert_eq!(cycle.current(), 0);
        assert_eq!(cycle.total(), 10);
    }
}
