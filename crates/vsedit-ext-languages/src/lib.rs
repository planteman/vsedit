//! Ext API: Language features.
//!
//! RPC bridge between the extension host and the main thread for languages.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_languages";

// ── RPC message types ──

/// Messages exchanged for the `languages` API surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LanguageMessage {
    RegisterCompletionProvider { registration: ProviderRegistration },
    RegisterHoverProvider { registration: ProviderRegistration },
    RegisterDefinitionProvider { registration: ProviderRegistration },
    RegisterDiagnostics { registration: ProviderRegistration },
    RegisterCodeActions { registration: ProviderRegistration },
    RegisterCodeLens { registration: ProviderRegistration },
    RegisterFormatter { registration: ProviderRegistration },
    RegisterSignatureHelp { registration: ProviderRegistration },
    RegisterRenameProvider { registration: ProviderRegistration },
    RegisterSymbolProvider { registration: ProviderRegistration },
}

/// A provider registration sent from the extension host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRegistration {
    pub provider_id: String,
    pub selector: LanguageSelector,
}

/// Document selector used to scope providers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSelector {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub scheme: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
}

/// Response payload for language provider operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LanguageResponse {
    Registered { handle: String },
}

// ── Feature classification ──

/// The kind of language feature a message represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LanguageFeatureKind {
    Completion,
    Hover,
    Definition,
    Diagnostics,
    CodeActions,
    CodeLens,
    Formatting,
    SignatureHelp,
    Rename,
    DocumentSymbol,
}

/// Returns the feature kind for a given language message.
pub fn get_feature_kind(msg: &LanguageMessage) -> LanguageFeatureKind {
    match msg {
        LanguageMessage::RegisterCompletionProvider { .. } => LanguageFeatureKind::Completion,
        LanguageMessage::RegisterHoverProvider { .. } => LanguageFeatureKind::Hover,
        LanguageMessage::RegisterDefinitionProvider { .. } => LanguageFeatureKind::Definition,
        LanguageMessage::RegisterDiagnostics { .. } => LanguageFeatureKind::Diagnostics,
        LanguageMessage::RegisterCodeActions { .. } => LanguageFeatureKind::CodeActions,
        LanguageMessage::RegisterCodeLens { .. } => LanguageFeatureKind::CodeLens,
        LanguageMessage::RegisterFormatter { .. } => LanguageFeatureKind::Formatting,
        LanguageMessage::RegisterSignatureHelp { .. } => LanguageFeatureKind::SignatureHelp,
        LanguageMessage::RegisterRenameProvider { .. } => LanguageFeatureKind::Rename,
        LanguageMessage::RegisterSymbolProvider { .. } => LanguageFeatureKind::DocumentSymbol,
    }
}

/// Aggregated statistics about registered providers.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderStats {
    pub total_providers: usize,
    pub providers_by_language: HashMap<String, usize>,
    pub providers_by_feature: HashMap<LanguageFeatureKind, usize>,
}

/// Check whether a `LanguageSelector` matches a given document described by
/// its language identifier, URI scheme, and file path. A `None` field in the
/// selector is treated as a wildcard (matches anything). The pattern field
/// performs a simple suffix/contains check rather than full glob evaluation.
pub fn selector_matches(
    selector: &LanguageSelector,
    language: &str,
    scheme: &str,
    path: &str,
) -> bool {
    if let Some(ref lang) = selector.language {
        if lang != language {
            return false;
        }
    }
    if let Some(ref s) = selector.scheme {
        if s != scheme {
            return false;
        }
    }
    if let Some(ref pat) = selector.pattern {
        let pat_trimmed = pat.trim_start_matches("**/");
        if pat_trimmed.starts_with("*.") {
            let ext = &pat_trimmed[1..]; // e.g. ".c"
            if !path.ends_with(ext) {
                return false;
            }
        } else if !path.ends_with(pat_trimmed) && !path.contains(pat_trimmed) {
            return false;
        }
    }
    true
}

// ── Bridge ──

/// Tracks registered language feature providers.
#[derive(Debug, Default)]
pub struct LanguageBridge {
    providers: HashMap<String, ProviderRegistration>,
    feature_kinds: HashMap<String, LanguageFeatureKind>,
}

impl LanguageBridge {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process an incoming language message and return a response.
    pub fn handle(&mut self, msg: LanguageMessage) -> LanguageResponse {
        let kind = get_feature_kind(&msg);
        let reg = match msg {
            LanguageMessage::RegisterCompletionProvider { registration }
            | LanguageMessage::RegisterHoverProvider { registration }
            | LanguageMessage::RegisterDefinitionProvider { registration }
            | LanguageMessage::RegisterDiagnostics { registration }
            | LanguageMessage::RegisterCodeActions { registration }
            | LanguageMessage::RegisterCodeLens { registration }
            | LanguageMessage::RegisterFormatter { registration }
            | LanguageMessage::RegisterSignatureHelp { registration }
            | LanguageMessage::RegisterRenameProvider { registration }
            | LanguageMessage::RegisterSymbolProvider { registration } => registration,
        };
        let handle = reg.provider_id.clone();
        self.providers.insert(handle.clone(), reg);
        self.feature_kinds.insert(handle.clone(), kind);
        LanguageResponse::Registered { handle }
    }

    /// Number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Look up a provider registration by ID.
    pub fn get_provider(&self, id: &str) -> Option<&ProviderRegistration> {
        self.providers.get(id)
    }

    /// Remove a provider by ID. Returns `true` if it existed.
    pub fn unregister_provider(&mut self, id: &str) -> bool {
        self.feature_kinds.remove(id);
        self.providers.remove(id).is_some()
    }

    /// Return all providers whose selector matches the given language.
    pub fn get_providers_for_language(&self, language: &str) -> Vec<&ProviderRegistration> {
        self.providers
            .values()
            .filter(|p| p.selector.language.as_deref() == Some(language))
            .collect()
    }

    /// Return all providers whose selector matches the given URI scheme.
    pub fn get_providers_for_scheme(&self, scheme: &str) -> Vec<&ProviderRegistration> {
        self.providers
            .values()
            .filter(|p| p.selector.scheme.as_deref() == Some(scheme))
            .collect()
    }

    /// Compute aggregate statistics over all registered providers.
    pub fn get_stats(&self) -> ProviderStats {
        let mut providers_by_language: HashMap<String, usize> = HashMap::new();
        let mut providers_by_feature: HashMap<LanguageFeatureKind, usize> = HashMap::new();
        for (id, reg) in &self.providers {
            if let Some(ref lang) = reg.selector.language {
                *providers_by_language.entry(lang.clone()).or_insert(0) += 1;
            }
            if let Some(&kind) = self.feature_kinds.get(id.as_str()) {
                *providers_by_feature.entry(kind).or_insert(0) += 1;
            }
        }
        ProviderStats {
            total_providers: self.providers.len(),
            providers_by_language,
            providers_by_feature,
        }
    }

    /// Return a sorted, deduplicated list of all languages across providers.
    pub fn get_all_languages(&self) -> Vec<String> {
        let mut langs: Vec<String> = self
            .providers
            .values()
            .filter_map(|p| p.selector.language.clone())
            .collect();
        langs.sort();
        langs.dedup();
        langs
    }
}

/// Initialize the languages extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

// ── Provider capabilities & feature support ──

/// Describes optional capabilities a language provider may support.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub supports_workspace_symbols: bool,
    pub supports_call_hierarchy: bool,
    pub supports_type_hierarchy: bool,
    pub supports_inlay_hints: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            supports_workspace_symbols: false,
            supports_call_hierarchy: false,
            supports_type_hierarchy: false,
            supports_inlay_hints: false,
        }
    }
}

/// Summary of which language features are available for a given language.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageFeatureSupport {
    pub has_completion: bool,
    pub has_hover: bool,
    pub has_definition: bool,
    pub has_diagnostics: bool,
    pub has_code_actions: bool,
    pub has_code_lens: bool,
    pub has_formatting: bool,
    pub has_signature_help: bool,
    pub has_rename: bool,
    pub has_document_symbol: bool,
}

/// Compute a [`LanguageFeatureSupport`] for `language` by inspecting all
/// providers registered in the given [`LanguageBridge`].
pub fn get_language_feature_support(bridge: &LanguageBridge, language: &str) -> LanguageFeatureSupport {
    let mut support = LanguageFeatureSupport {
        has_completion: false,
        has_hover: false,
        has_definition: false,
        has_diagnostics: false,
        has_code_actions: false,
        has_code_lens: false,
        has_formatting: false,
        has_signature_help: false,
        has_rename: false,
        has_document_symbol: false,
    };
    for (id, reg) in &bridge.providers {
        if reg.selector.language.as_deref() != Some(language) {
            continue;
        }
        if let Some(&kind) = bridge.feature_kinds.get(id.as_str()) {
            match kind {
                LanguageFeatureKind::Completion => support.has_completion = true,
                LanguageFeatureKind::Hover => support.has_hover = true,
                LanguageFeatureKind::Definition => support.has_definition = true,
                LanguageFeatureKind::Diagnostics => support.has_diagnostics = true,
                LanguageFeatureKind::CodeActions => support.has_code_actions = true,
                LanguageFeatureKind::CodeLens => support.has_code_lens = true,
                LanguageFeatureKind::Formatting => support.has_formatting = true,
                LanguageFeatureKind::SignatureHelp => support.has_signature_help = true,
                LanguageFeatureKind::Rename => support.has_rename = true,
                LanguageFeatureKind::DocumentSymbol => support.has_document_symbol = true,
            }
        }
    }
    support
}

// ── Provider priority ──

/// Priority level used to order multiple providers for the same feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderPriority {
    /// Normal priority (default).
    Default,
    /// Elevated priority — preferred over `Default` providers.
    High,
    /// Only this provider should be used; all others are suppressed.
    Exclusive,
}

impl ProviderPriority {
    /// Numeric weight for sorting (higher = more important).
    pub fn weight(self) -> u32 {
        match self {
            ProviderPriority::Default => 0,
            ProviderPriority::High => 1,
            ProviderPriority::Exclusive => 2,
        }
    }
}

// ── Selector scoring ──

/// Score how specifically a [`LanguageSelector`] matches a document described
/// by `language`, `scheme`, and `path`. Returns `0` if the selector does not
/// match at all; otherwise a higher score means a more specific match.
pub fn selector_score(
    selector: &LanguageSelector,
    language: &str,
    scheme: &str,
    path: &str,
) -> u32 {
    if !selector_matches(selector, language, scheme, path) {
        return 0;
    }
    let mut score: u32 = 1; // baseline for a wildcard-only match
    if selector.language.is_some() {
        score += 10;
    }
    if selector.scheme.is_some() {
        score += 5;
    }
    if selector.pattern.is_some() {
        score += 3;
    }
    score
}

// ── Formatting helpers ──

/// Produce a human-readable summary of every provider in the bridge,
/// grouped by language. The output is sorted by language name.
pub fn format_provider_summary(bridge: &LanguageBridge) -> String {
    let mut by_lang: HashMap<String, Vec<String>> = HashMap::new();
    for (id, reg) in &bridge.providers {
        let lang = reg
            .selector
            .language
            .clone()
            .unwrap_or_else(|| "*".to_string());
        let kind_label = bridge
            .feature_kinds
            .get(id.as_str())
            .map(|k| format!("{:?}", k))
            .unwrap_or_else(|| "Unknown".to_string());
        by_lang
            .entry(lang)
            .or_default()
            .push(format!("{} ({})", id, kind_label));
    }
    let mut langs: Vec<String> = by_lang.keys().cloned().collect();
    langs.sort();
    let mut out = String::new();
    for lang in &langs {
        out.push_str(&format!("[{}]\n", lang));
        let entries = by_lang.get(lang).unwrap();
        let mut sorted_entries = entries.clone();
        sorted_entries.sort();
        for entry in &sorted_entries {
            out.push_str(&format!("  {}\n", entry));
        }
    }
    out
}

// ── Selector builder ──

/// Fluent builder for constructing a [`LanguageSelector`].
pub struct LanguageSelectorBuilder {
    language: Option<String>,
    scheme: Option<String>,
    pattern: Option<String>,
}

impl LanguageSelectorBuilder {
    pub fn new() -> Self {
        Self {
            language: None,
            scheme: None,
            pattern: None,
        }
    }

    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn scheme(mut self, scheme: impl Into<String>) -> Self {
        self.scheme = Some(scheme.into());
        self
    }

    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    pub fn build(self) -> LanguageSelector {
        LanguageSelector {
            language: self.language,
            scheme: self.scheme,
            pattern: self.pattern,
        }
    }
}

impl Default for LanguageSelectorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ── Provider ranking ──

/// A provider with its computed score for a given document.
#[derive(Debug, Clone, PartialEq)]
pub struct RankedProvider {
    pub provider_id: String,
    pub score: u32,
}

/// Ranks multiple providers by their selector score for a given document,
/// returning them in descending score order. Providers with a score of 0
/// (no match) are excluded.
pub struct ProviderRanking;

impl ProviderRanking {
    pub fn rank(
        providers: &[ProviderRegistration],
        language: &str,
        scheme: &str,
        path: &str,
    ) -> Vec<RankedProvider> {
        let mut ranked: Vec<RankedProvider> = providers
            .iter()
            .filter_map(|p| {
                let score = selector_score(&p.selector, language, scheme, path);
                if score > 0 {
                    Some(RankedProvider {
                        provider_id: p.provider_id.clone(),
                        score,
                    })
                } else {
                    None
                }
            })
            .collect();
        ranked.sort_by(|a, b| b.score.cmp(&a.score));
        ranked
    }

    /// Returns the single best-matching provider, or `None` if none match.
    pub fn best(
        providers: &[ProviderRegistration],
        language: &str,
        scheme: &str,
        path: &str,
    ) -> Option<RankedProvider> {
        Self::rank(providers, language, scheme, path).into_iter().next()
    }
}

// ── Language status ──

/// Severity level for a language status item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LanguageStatusSeverity {
    Information,
    Warning,
    Error,
}

/// Status bar information for a specific language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageStatus {
    pub language_id: String,
    pub label: String,
    pub detail: Option<String>,
    pub severity: LanguageStatusSeverity,
    pub busy: bool,
}

impl LanguageStatus {
    pub fn new(language_id: &str, label: &str) -> Self {
        Self {
            language_id: language_id.to_string(),
            label: label.to_string(),
            detail: None,
            severity: LanguageStatusSeverity::Information,
            busy: false,
        }
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_severity(mut self, severity: LanguageStatusSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    pub fn display_text(&self) -> String {
        match &self.detail {
            Some(d) => format!("{} ({})", self.label, d),
            None => self.label.clone(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == LanguageStatusSeverity::Error
    }
}

// ── Completion item converter ──

/// Converts between VSCode completion item kind numbers and string names.
pub struct CompletionItemConverter;

impl CompletionItemConverter {
    /// Maps a VSCode completion kind number to its string name.
    pub fn kind_to_string(kind: u32) -> &'static str {
        match kind {
            1 => "Method",
            2 => "Function",
            3 => "Constructor",
            4 => "Field",
            5 => "Variable",
            6 => "Class",
            7 => "Interface",
            8 => "Module",
            9 => "Property",
            10 => "Unit",
            11 => "Value",
            12 => "Enum",
            13 => "Keyword",
            14 => "Snippet",
            15 => "Color",
            16 => "File",
            17 => "Reference",
            18 => "Folder",
            19 => "EnumMember",
            20 => "Constant",
            21 => "Struct",
            22 => "Event",
            23 => "Operator",
            24 => "TypeParameter",
            _ => "Unknown",
        }
    }

    /// Maps a string name back to its VSCode completion kind number.
    pub fn string_to_kind(s: &str) -> Option<u32> {
        match s {
            "Method" => Some(1),
            "Function" => Some(2),
            "Constructor" => Some(3),
            "Field" => Some(4),
            "Variable" => Some(5),
            "Class" => Some(6),
            "Interface" => Some(7),
            "Module" => Some(8),
            "Property" => Some(9),
            "Unit" => Some(10),
            "Value" => Some(11),
            "Enum" => Some(12),
            "Keyword" => Some(13),
            "Snippet" => Some(14),
            "Color" => Some(15),
            "File" => Some(16),
            "Reference" => Some(17),
            "Folder" => Some(18),
            "EnumMember" => Some(19),
            "Constant" => Some(20),
            "Struct" => Some(21),
            "Event" => Some(22),
            "Operator" => Some(23),
            "TypeParameter" => Some(24),
            _ => None,
        }
    }

    /// Returns an icon character for the given completion kind.
    pub fn icon_for_kind(kind: u32) -> &'static str {
        match kind {
            1 => "ƒ",  // Method
            2 => "ƒ",  // Function
            3 => "⊕",  // Constructor
            4 => "□",  // Field
            5 => "𝑥",  // Variable
            6 => "◆",  // Class
            7 => "◇",  // Interface
            8 => "▣",  // Module
            9 => "◫",  // Property
            10 => "∪", // Unit
            11 => "=",  // Value
            12 => "∈", // Enum
            13 => "⌘", // Keyword
            14 => "✂",  // Snippet
            15 => "◉", // Color
            16 => "📄", // File
            17 => "↗",  // Reference
            18 => "📁", // Folder
            19 => "∊", // EnumMember
            20 => "π",  // Constant
            21 => "▧", // Struct
            22 => "⚡", // Event
            23 => "±", // Operator
            24 => "τ",  // TypeParameter
            _ => "?",
        }
    }

    /// Returns a sort prefix string that orders kinds logically:
    /// methods/functions first, then fields/properties, then variables, etc.
    pub fn sort_text_for_kind(kind: u32) -> String {
        let prefix = match kind {
            1 => "aa", // Method
            2 => "ab", // Function
            3 => "ac", // Constructor
            4 => "ba", // Field
            9 => "bb", // Property
            5 => "ca", // Variable
            20 => "cb", // Constant
            6 => "da", // Class
            21 => "db", // Struct
            7 => "dc", // Interface
            12 => "dd", // Enum
            19 => "de", // EnumMember
            8 => "ea", // Module
            13 => "fa", // Keyword
            14 => "fb", // Snippet
            24 => "ga", // TypeParameter
            _ => "zz",
        };
        prefix.to_string()
    }
}

// ── Feature support query ──

/// Returns `true` if at least one provider registered in `bridge` for the
/// given `language` supports the specified `feature`.
pub fn language_feature_supported(
    bridge: &LanguageBridge,
    language: &str,
    feature: LanguageFeatureKind,
) -> bool {
    for (id, reg) in &bridge.providers {
        if reg.selector.language.as_deref() != Some(language) {
            continue;
        }
        if let Some(&kind) = bridge.feature_kinds.get(id.as_str()) {
            if kind == feature {
                return true;
            }
        }
    }
    false
}

// ── Language status registry ──

/// Registry for tracking language status items across multiple languages.
#[derive(Debug, Default)]
pub struct LanguageStatusRegistry {
    statuses: HashMap<String, LanguageStatus>,
}

impl LanguageStatusRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or update a status for a language.
    pub fn set_status(&mut self, status: LanguageStatus) {
        self.statuses.insert(status.language_id.clone(), status);
    }

    /// Retrieve the status for a language, if any.
    pub fn get_status(&self, language_id: &str) -> Option<&LanguageStatus> {
        self.statuses.get(language_id)
    }

    /// Remove a language status. Returns `true` if it existed.
    pub fn remove_status(&mut self, language_id: &str) -> bool {
        self.statuses.remove(language_id).is_some()
    }

    /// Return all statuses in the registry.
    pub fn all_statuses(&self) -> Vec<&LanguageStatus> {
        self.statuses.values().collect()
    }

    /// Return language IDs that have error severity.
    pub fn error_languages(&self) -> Vec<&str> {
        self.statuses
            .values()
            .filter(|s| s.severity == LanguageStatusSeverity::Error)
            .map(|s| s.language_id.as_str())
            .collect()
    }

    /// Return language IDs that are marked busy.
    pub fn busy_languages(&self) -> Vec<&str> {
        self.statuses
            .values()
            .filter(|s| s.busy)
            .map(|s| s.language_id.as_str())
            .collect()
    }

    /// Number of tracked language statuses.
    pub fn count(&self) -> usize {
        self.statuses.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selector(lang: &str) -> LanguageSelector {
        LanguageSelector { language: Some(lang.into()), scheme: None, pattern: None }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn register_completion_provider() {
        let mut bridge = LanguageBridge::new();
        let resp = bridge.handle(LanguageMessage::RegisterCompletionProvider {
            registration: ProviderRegistration {
                provider_id: "comp-1".into(),
                selector: selector("rust"),
            },
        });
        assert_eq!(resp, LanguageResponse::Registered { handle: "comp-1".into() });
        assert_eq!(bridge.provider_count(), 1);
    }

    #[test]
    fn register_multiple_providers() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterHoverProvider {
            registration: ProviderRegistration {
                provider_id: "hover-1".into(),
                selector: selector("python"),
            },
        });
        bridge.handle(LanguageMessage::RegisterDefinitionProvider {
            registration: ProviderRegistration {
                provider_id: "def-1".into(),
                selector: selector("python"),
            },
        });
        assert_eq!(bridge.provider_count(), 2);
    }

    #[test]
    fn get_provider_by_id() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterFormatter {
            registration: ProviderRegistration {
                provider_id: "fmt-1".into(),
                selector: selector("typescript"),
            },
        });
        let reg = bridge.get_provider("fmt-1").unwrap();
        assert_eq!(reg.selector.language.as_deref(), Some("typescript"));
    }

    #[test]
    fn overwrite_same_id() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterCodeLens {
            registration: ProviderRegistration {
                provider_id: "cl-1".into(),
                selector: selector("go"),
            },
        });
        bridge.handle(LanguageMessage::RegisterCodeLens {
            registration: ProviderRegistration {
                provider_id: "cl-1".into(),
                selector: selector("java"),
            },
        });
        assert_eq!(bridge.provider_count(), 1);
        assert_eq!(bridge.get_provider("cl-1").unwrap().selector.language.as_deref(), Some("java"));
    }

    #[test]
    fn serde_round_trip() {
        let msg = LanguageMessage::RegisterRenameProvider {
            registration: ProviderRegistration {
                provider_id: "ren-1".into(),
                selector: LanguageSelector {
                    language: Some("c".into()),
                    scheme: Some("file".into()),
                    pattern: Some("**/*.c".into()),
                },
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: LanguageMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    // ── New tests ──

    #[test]
    fn unregister_provider_existing() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterCompletionProvider {
            registration: ProviderRegistration {
                provider_id: "comp-x".into(),
                selector: selector("rust"),
            },
        });
        assert!(bridge.unregister_provider("comp-x"));
        assert_eq!(bridge.provider_count(), 0);
        assert!(bridge.get_provider("comp-x").is_none());
    }

    #[test]
    fn unregister_provider_missing() {
        let mut bridge = LanguageBridge::new();
        assert!(!bridge.unregister_provider("nonexistent"));
    }

    #[test]
    fn get_providers_for_language_filters() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterCompletionProvider {
            registration: ProviderRegistration {
                provider_id: "c1".into(),
                selector: selector("rust"),
            },
        });
        bridge.handle(LanguageMessage::RegisterHoverProvider {
            registration: ProviderRegistration {
                provider_id: "h1".into(),
                selector: selector("rust"),
            },
        });
        bridge.handle(LanguageMessage::RegisterDefinitionProvider {
            registration: ProviderRegistration {
                provider_id: "d1".into(),
                selector: selector("python"),
            },
        });
        let rust_providers = bridge.get_providers_for_language("rust");
        assert_eq!(rust_providers.len(), 2);
        assert!(bridge.get_providers_for_language("java").is_empty());
    }

    #[test]
    fn get_providers_for_scheme_filters() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterFormatter {
            registration: ProviderRegistration {
                provider_id: "f1".into(),
                selector: LanguageSelector {
                    language: Some("rust".into()),
                    scheme: Some("file".into()),
                    pattern: None,
                },
            },
        });
        bridge.handle(LanguageMessage::RegisterFormatter {
            registration: ProviderRegistration {
                provider_id: "f2".into(),
                selector: LanguageSelector {
                    language: Some("go".into()),
                    scheme: Some("untitled".into()),
                    pattern: None,
                },
            },
        });
        assert_eq!(bridge.get_providers_for_scheme("file").len(), 1);
        assert_eq!(bridge.get_providers_for_scheme("untitled").len(), 1);
        assert!(bridge.get_providers_for_scheme("vscode").is_empty());
    }

    #[test]
    fn feature_kind_mapping() {
        let msg = LanguageMessage::RegisterCompletionProvider {
            registration: ProviderRegistration {
                provider_id: "x".into(),
                selector: selector("rust"),
            },
        };
        assert_eq!(get_feature_kind(&msg), LanguageFeatureKind::Completion);

        let msg2 = LanguageMessage::RegisterSymbolProvider {
            registration: ProviderRegistration {
                provider_id: "y".into(),
                selector: selector("go"),
            },
        };
        assert_eq!(get_feature_kind(&msg2), LanguageFeatureKind::DocumentSymbol);
    }

    #[test]
    fn get_stats_aggregates() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterCompletionProvider {
            registration: ProviderRegistration {
                provider_id: "c1".into(),
                selector: selector("rust"),
            },
        });
        bridge.handle(LanguageMessage::RegisterCompletionProvider {
            registration: ProviderRegistration {
                provider_id: "c2".into(),
                selector: selector("rust"),
            },
        });
        bridge.handle(LanguageMessage::RegisterHoverProvider {
            registration: ProviderRegistration {
                provider_id: "h1".into(),
                selector: selector("python"),
            },
        });
        let stats = bridge.get_stats();
        assert_eq!(stats.total_providers, 3);
        assert_eq!(stats.providers_by_language.get("rust"), Some(&2));
        assert_eq!(stats.providers_by_language.get("python"), Some(&1));
        assert_eq!(
            stats.providers_by_feature.get(&LanguageFeatureKind::Completion),
            Some(&2)
        );
        assert_eq!(
            stats.providers_by_feature.get(&LanguageFeatureKind::Hover),
            Some(&1)
        );
    }

    #[test]
    fn selector_matches_all_wildcard() {
        let sel = LanguageSelector { language: None, scheme: None, pattern: None };
        assert!(selector_matches(&sel, "rust", "file", "/a/b.rs"));
    }

    #[test]
    fn selector_matches_language_mismatch() {
        let sel = LanguageSelector {
            language: Some("python".into()),
            scheme: None,
            pattern: None,
        };
        assert!(!selector_matches(&sel, "rust", "file", "/a/b.rs"));
    }

    #[test]
    fn selector_matches_scheme_and_pattern() {
        let sel = LanguageSelector {
            language: Some("c".into()),
            scheme: Some("file".into()),
            pattern: Some("**/*.c".into()),
        };
        assert!(selector_matches(&sel, "c", "file", "/src/main.c"));
        assert!(!selector_matches(&sel, "c", "untitled", "/src/main.c"));
        assert!(!selector_matches(&sel, "c", "file", "/src/main.h"));
    }

    #[test]
    fn get_all_languages_unique_sorted() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterCompletionProvider {
            registration: ProviderRegistration {
                provider_id: "a".into(),
                selector: selector("rust"),
            },
        });
        bridge.handle(LanguageMessage::RegisterHoverProvider {
            registration: ProviderRegistration {
                provider_id: "b".into(),
                selector: selector("python"),
            },
        });
        bridge.handle(LanguageMessage::RegisterDefinitionProvider {
            registration: ProviderRegistration {
                provider_id: "c".into(),
                selector: selector("rust"),
            },
        });
        bridge.handle(LanguageMessage::RegisterFormatter {
            registration: ProviderRegistration {
                provider_id: "d".into(),
                selector: selector("go"),
            },
        });
        let langs = bridge.get_all_languages();
        assert_eq!(langs, vec!["go", "python", "rust"]);
    }

    #[test]
    fn feature_kind_serde_round_trip() {
        let kind = LanguageFeatureKind::SignatureHelp;
        let json = serde_json::to_string(&kind).unwrap();
        let parsed: LanguageFeatureKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, parsed);
    }

    // ── New tests for added types & functions ──

    #[test]
    fn provider_capabilities_default() {
        let caps = ProviderCapabilities::default();
        assert!(!caps.supports_workspace_symbols);
        assert!(!caps.supports_call_hierarchy);
        assert!(!caps.supports_type_hierarchy);
        assert!(!caps.supports_inlay_hints);
    }

    #[test]
    fn provider_capabilities_serde_round_trip() {
        let caps = ProviderCapabilities {
            supports_workspace_symbols: true,
            supports_call_hierarchy: false,
            supports_type_hierarchy: true,
            supports_inlay_hints: false,
        };
        let json = serde_json::to_string(&caps).unwrap();
        let parsed: ProviderCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, parsed);
    }

    #[test]
    fn language_feature_support_empty_bridge() {
        let bridge = LanguageBridge::new();
        let support = get_language_feature_support(&bridge, "rust");
        assert!(!support.has_completion);
        assert!(!support.has_hover);
        assert!(!support.has_definition);
        assert!(!support.has_formatting);
    }

    #[test]
    fn language_feature_support_partial() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterCompletionProvider {
            registration: ProviderRegistration {
                provider_id: "c1".into(),
                selector: selector("rust"),
            },
        });
        bridge.handle(LanguageMessage::RegisterHoverProvider {
            registration: ProviderRegistration {
                provider_id: "h1".into(),
                selector: selector("rust"),
            },
        });
        bridge.handle(LanguageMessage::RegisterFormatter {
            registration: ProviderRegistration {
                provider_id: "f1".into(),
                selector: selector("python"),
            },
        });
        let rust_support = get_language_feature_support(&bridge, "rust");
        assert!(rust_support.has_completion);
        assert!(rust_support.has_hover);
        assert!(!rust_support.has_definition);
        assert!(!rust_support.has_formatting);

        let py_support = get_language_feature_support(&bridge, "python");
        assert!(!py_support.has_completion);
        assert!(py_support.has_formatting);
    }

    #[test]
    fn provider_priority_weight_ordering() {
        assert!(ProviderPriority::High.weight() > ProviderPriority::Default.weight());
        assert!(ProviderPriority::Exclusive.weight() > ProviderPriority::High.weight());
    }

    #[test]
    fn provider_priority_serde_round_trip() {
        let prio = ProviderPriority::Exclusive;
        let json = serde_json::to_string(&prio).unwrap();
        let parsed: ProviderPriority = serde_json::from_str(&json).unwrap();
        assert_eq!(prio, parsed);
    }

    #[test]
    fn selector_score_no_match_returns_zero() {
        let sel = LanguageSelector {
            language: Some("python".into()),
            scheme: None,
            pattern: None,
        };
        assert_eq!(selector_score(&sel, "rust", "file", "/a/b.rs"), 0);
    }

    #[test]
    fn selector_score_wildcard_baseline() {
        let sel = LanguageSelector { language: None, scheme: None, pattern: None };
        assert_eq!(selector_score(&sel, "rust", "file", "/a/b.rs"), 1);
    }

    #[test]
    fn selector_score_specificity_increases() {
        let lang_only = LanguageSelector {
            language: Some("rust".into()),
            scheme: None,
            pattern: None,
        };
        let lang_scheme = LanguageSelector {
            language: Some("rust".into()),
            scheme: Some("file".into()),
            pattern: None,
        };
        let full = LanguageSelector {
            language: Some("rust".into()),
            scheme: Some("file".into()),
            pattern: Some("**/*.rs".into()),
        };
        let s1 = selector_score(&lang_only, "rust", "file", "/a/b.rs");
        let s2 = selector_score(&lang_scheme, "rust", "file", "/a/b.rs");
        let s3 = selector_score(&full, "rust", "file", "/a/b.rs");
        assert!(s1 < s2);
        assert!(s2 < s3);
    }

    #[test]
    fn format_provider_summary_output() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterCompletionProvider {
            registration: ProviderRegistration {
                provider_id: "comp-1".into(),
                selector: selector("rust"),
            },
        });
        bridge.handle(LanguageMessage::RegisterHoverProvider {
            registration: ProviderRegistration {
                provider_id: "hov-1".into(),
                selector: selector("rust"),
            },
        });
        bridge.handle(LanguageMessage::RegisterFormatter {
            registration: ProviderRegistration {
                provider_id: "fmt-1".into(),
                selector: selector("go"),
            },
        });
        let summary = format_provider_summary(&bridge);
        assert!(summary.contains("[go]"));
        assert!(summary.contains("[rust]"));
        assert!(summary.contains("comp-1"));
        assert!(summary.contains("hov-1"));
        assert!(summary.contains("fmt-1"));
        // go section should appear before rust (sorted)
        let go_pos = summary.find("[go]").unwrap();
        let rust_pos = summary.find("[rust]").unwrap();
        assert!(go_pos < rust_pos);
    }

    // ── LanguageSelectorBuilder tests ──

    #[test]
    fn selector_builder_all_fields() {
        let sel = LanguageSelectorBuilder::new()
            .language("rust")
            .scheme("file")
            .pattern("**/*.rs")
            .build();
        assert_eq!(sel.language.as_deref(), Some("rust"));
        assert_eq!(sel.scheme.as_deref(), Some("file"));
        assert_eq!(sel.pattern.as_deref(), Some("**/*.rs"));
    }

    #[test]
    fn selector_builder_partial() {
        let sel = LanguageSelectorBuilder::new().language("go").build();
        assert_eq!(sel.language.as_deref(), Some("go"));
        assert!(sel.scheme.is_none());
        assert!(sel.pattern.is_none());
    }

    #[test]
    fn selector_builder_default_is_wildcard() {
        let sel = LanguageSelectorBuilder::default().build();
        assert!(sel.language.is_none());
        assert!(sel.scheme.is_none());
        assert!(sel.pattern.is_none());
        assert!(selector_matches(&sel, "any", "any", "/any/path"));
    }

    // ── ProviderRanking tests ──

    #[test]
    fn ranking_orders_by_score_descending() {
        let providers = vec![
            ProviderRegistration {
                provider_id: "wildcard".into(),
                selector: LanguageSelectorBuilder::new().build(),
            },
            ProviderRegistration {
                provider_id: "lang-only".into(),
                selector: LanguageSelectorBuilder::new().language("rust").build(),
            },
            ProviderRegistration {
                provider_id: "full".into(),
                selector: LanguageSelectorBuilder::new()
                    .language("rust")
                    .scheme("file")
                    .pattern("**/*.rs")
                    .build(),
            },
        ];
        let ranked = ProviderRanking::rank(&providers, "rust", "file", "/src/main.rs");
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].provider_id, "full");
        assert_eq!(ranked[1].provider_id, "lang-only");
        assert_eq!(ranked[2].provider_id, "wildcard");
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn ranking_excludes_non_matching() {
        let providers = vec![
            ProviderRegistration {
                provider_id: "py".into(),
                selector: LanguageSelectorBuilder::new().language("python").build(),
            },
        ];
        let ranked = ProviderRanking::rank(&providers, "rust", "file", "/a.rs");
        assert!(ranked.is_empty());
    }

    #[test]
    fn ranking_best_returns_top() {
        let providers = vec![
            ProviderRegistration {
                provider_id: "a".into(),
                selector: LanguageSelectorBuilder::new().build(),
            },
            ProviderRegistration {
                provider_id: "b".into(),
                selector: LanguageSelectorBuilder::new().language("rust").build(),
            },
        ];
        let best = ProviderRanking::best(&providers, "rust", "file", "/a.rs").unwrap();
        assert_eq!(best.provider_id, "b");
    }

    #[test]
    fn ranking_best_returns_none_when_empty() {
        let best = ProviderRanking::best(&[], "rust", "file", "/a.rs");
        assert!(best.is_none());
    }

    // ── LanguageStatus tests ──

    #[test]
    fn language_status_creation_and_display_text() {
        let status = LanguageStatus::new("rust", "Rust Analyzer");
        assert_eq!(status.language_id, "rust");
        assert_eq!(status.label, "Rust Analyzer");
        assert_eq!(status.severity, LanguageStatusSeverity::Information);
        assert!(!status.busy);
        assert_eq!(status.display_text(), "Rust Analyzer");
    }

    #[test]
    fn language_status_with_detail() {
        let status = LanguageStatus::new("python", "Pylance")
            .with_detail("loading workspace");
        assert_eq!(status.detail, Some("loading workspace".to_string()));
        assert_eq!(status.display_text(), "Pylance (loading workspace)");
    }

    #[test]
    fn language_status_is_error() {
        let info = LanguageStatus::new("go", "gopls");
        assert!(!info.is_error());

        let err = LanguageStatus::new("go", "gopls")
            .with_severity(LanguageStatusSeverity::Error);
        assert!(err.is_error());

        let warn = LanguageStatus::new("go", "gopls")
            .with_severity(LanguageStatusSeverity::Warning);
        assert!(!warn.is_error());
    }

    // ── CompletionItemConverter tests ──

    #[test]
    fn completion_converter_kind_to_string() {
        assert_eq!(CompletionItemConverter::kind_to_string(1), "Method");
        assert_eq!(CompletionItemConverter::kind_to_string(2), "Function");
        assert_eq!(CompletionItemConverter::kind_to_string(6), "Class");
        assert_eq!(CompletionItemConverter::kind_to_string(13), "Keyword");
        assert_eq!(CompletionItemConverter::kind_to_string(99), "Unknown");
    }

    #[test]
    fn completion_converter_string_to_kind() {
        assert_eq!(CompletionItemConverter::string_to_kind("Method"), Some(1));
        assert_eq!(CompletionItemConverter::string_to_kind("Variable"), Some(5));
        assert_eq!(CompletionItemConverter::string_to_kind("Struct"), Some(21));
        assert_eq!(CompletionItemConverter::string_to_kind("Bogus"), None);
    }

    #[test]
    fn completion_converter_icon_for_kind() {
        assert_eq!(CompletionItemConverter::icon_for_kind(1), "ƒ");
        assert_eq!(CompletionItemConverter::icon_for_kind(6), "◆");
        assert_eq!(CompletionItemConverter::icon_for_kind(13), "⌘");
        assert_eq!(CompletionItemConverter::icon_for_kind(255), "?");
    }

    // ── language_feature_supported tests ──

    #[test]
    fn language_feature_supported_returns_true_when_provider_exists() {
        let mut bridge = LanguageBridge::new();
        bridge.handle(LanguageMessage::RegisterCompletionProvider {
            registration: ProviderRegistration {
                provider_id: "rust-compl".into(),
                selector: selector("rust"),
            },
        });
        assert!(language_feature_supported(&bridge, "rust", LanguageFeatureKind::Completion));
        assert!(!language_feature_supported(&bridge, "rust", LanguageFeatureKind::Hover));
        assert!(!language_feature_supported(&bridge, "python", LanguageFeatureKind::Completion));
    }

    // ── LanguageStatusRegistry tests ──

    #[test]
    fn status_registry_set_and_get() {
        let mut reg = LanguageStatusRegistry::new();
        assert_eq!(reg.count(), 0);

        reg.set_status(LanguageStatus::new("rust", "RA"));
        assert_eq!(reg.count(), 1);
        let s = reg.get_status("rust").unwrap();
        assert_eq!(s.label, "RA");

        assert!(reg.get_status("python").is_none());
    }

    #[test]
    fn status_registry_error_languages() {
        let mut reg = LanguageStatusRegistry::new();
        reg.set_status(LanguageStatus::new("rust", "RA"));
        reg.set_status(
            LanguageStatus::new("python", "Pylance")
                .with_severity(LanguageStatusSeverity::Error),
        );
        reg.set_status(
            LanguageStatus::new("go", "gopls")
                .with_severity(LanguageStatusSeverity::Warning),
        );

        let errors = reg.error_languages();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0], "python");
    }

    #[test]
    fn status_registry_busy_languages_and_remove() {
        let mut reg = LanguageStatusRegistry::new();
        let mut status = LanguageStatus::new("typescript", "tsserver");
        status.set_busy(true);
        reg.set_status(status);
        reg.set_status(LanguageStatus::new("css", "css-lsp"));

        let busy = reg.busy_languages();
        assert_eq!(busy.len(), 1);
        assert_eq!(busy[0], "typescript");

        assert!(reg.remove_status("typescript"));
        assert!(!reg.remove_status("typescript"));
        assert_eq!(reg.count(), 1);
    }
}
