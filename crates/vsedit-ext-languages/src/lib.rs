//! Ext API: Language features.
//!
//! RPC bridge between the extension host and the main thread for languages.

use std::collections::HashMap;
use std::fmt;

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

// ── Display impls ──

impl fmt::Display for LanguageFeatureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            LanguageFeatureKind::Completion => "Completion",
            LanguageFeatureKind::Hover => "Hover",
            LanguageFeatureKind::Definition => "Definition",
            LanguageFeatureKind::Diagnostics => "Diagnostics",
            LanguageFeatureKind::CodeActions => "Code Actions",
            LanguageFeatureKind::CodeLens => "Code Lens",
            LanguageFeatureKind::Formatting => "Formatting",
            LanguageFeatureKind::SignatureHelp => "Signature Help",
            LanguageFeatureKind::Rename => "Rename",
            LanguageFeatureKind::DocumentSymbol => "Document Symbol",
        };
        write!(f, "{}", label)
    }
}

impl fmt::Display for ProviderPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            ProviderPriority::Default => "default",
            ProviderPriority::High => "high",
            ProviderPriority::Exclusive => "exclusive",
        };
        write!(f, "{}", label)
    }
}

impl fmt::Display for LanguageStatusSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            LanguageStatusSeverity::Information => "info",
            LanguageStatusSeverity::Warning => "warning",
            LanguageStatusSeverity::Error => "error",
        };
        write!(f, "{}", label)
    }
}

impl fmt::Display for LanguageSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lang = self.language.as_deref().unwrap_or("*");
        let scheme = self.scheme.as_deref().unwrap_or("*");
        let pat = self.pattern.as_deref().unwrap_or("*");
        write!(f, "{}:{}:{}", lang, scheme, pat)
    }
}

impl fmt::Display for LanguageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} ({})", self.severity, self.display_text(), self.language_id)
    }
}

// ── From impls ──

impl From<&str> for LanguageSelector {
    fn from(language: &str) -> Self {
        LanguageSelector {
            language: Some(language.to_string()),
            scheme: None,
            pattern: None,
        }
    }
}

impl From<ProviderPriority> for u32 {
    fn from(p: ProviderPriority) -> u32 {
        p.weight()
    }
}

// ── SelectorMatcher ──

/// Evaluates whether a [`LanguageSelector`] matches a document described by
/// its language, scheme, and file path. Wraps the matching logic with document
/// metadata so it can be reused across multiple selectors without repeating
/// the document fields.
#[derive(Debug, Clone)]
pub struct SelectorMatcher {
    pub language: String,
    pub scheme: String,
    pub path: String,
}

impl SelectorMatcher {
    pub fn new(language: impl Into<String>, scheme: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            language: language.into(),
            scheme: scheme.into(),
            path: path.into(),
        }
    }

    /// Returns `true` if the given selector matches this document.
    pub fn matches(&self, selector: &LanguageSelector) -> bool {
        selector_matches(selector, &self.language, &self.scheme, &self.path)
    }

    /// Returns the specificity score for the given selector against this document.
    pub fn score(&self, selector: &LanguageSelector) -> u32 {
        selector_score(selector, &self.language, &self.scheme, &self.path)
    }

    /// Filters a slice of selectors, returning only those that match.
    pub fn filter_matching<'a>(&self, selectors: &'a [LanguageSelector]) -> Vec<&'a LanguageSelector> {
        selectors.iter().filter(|s| self.matches(s)).collect()
    }

    /// Returns the best-matching selector from a slice, or `None` if none match.
    pub fn best_match<'a>(&self, selectors: &'a [LanguageSelector]) -> Option<&'a LanguageSelector> {
        selectors
            .iter()
            .filter(|s| self.matches(s))
            .max_by_key(|s| self.score(s))
    }
}

// ── FeatureMatrix ──

/// Tracks which [`LanguageFeatureKind`]s are available for which languages,
/// providing summary queries across the entire matrix.
#[derive(Debug, Clone, Default)]
pub struct FeatureMatrix {
    entries: HashMap<String, Vec<LanguageFeatureKind>>,
}

impl FeatureMatrix {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a `FeatureMatrix` from a `LanguageBridge` by inspecting all
    /// registered providers.
    pub fn from_bridge(bridge: &LanguageBridge) -> Self {
        let mut matrix = Self::new();
        for (id, reg) in &bridge.providers {
            if let (Some(lang), Some(&kind)) =
                (&reg.selector.language, bridge.feature_kinds.get(id.as_str()))
            {
                matrix.add(lang, kind);
            }
        }
        matrix
    }

    /// Record that `language` supports `feature`.
    pub fn add(&mut self, language: &str, feature: LanguageFeatureKind) {
        let features = self.entries.entry(language.to_string()).or_default();
        if !features.contains(&feature) {
            features.push(feature);
        }
    }

    /// Returns the set of features available for a language.
    pub fn features_for(&self, language: &str) -> &[LanguageFeatureKind] {
        self.entries.get(language).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns all languages that have at least one feature registered.
    pub fn languages(&self) -> Vec<&str> {
        let mut langs: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        langs.sort();
        langs
    }

    /// Number of distinct (language, feature) pairs.
    pub fn total_entries(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }

    /// Returns languages that support every feature in `required`.
    pub fn languages_with_all(&self, required: &[LanguageFeatureKind]) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, features)| required.iter().all(|r| features.contains(r)))
            .map(|(lang, _)| lang.as_str())
            .collect()
    }

    /// Returns languages that support at least one feature in `any`.
    pub fn languages_with_any(&self, any: &[LanguageFeatureKind]) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, features)| any.iter().any(|r| features.contains(r)))
            .map(|(lang, _)| lang.as_str())
            .collect()
    }

    /// Returns a human-readable summary of the matrix.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        let mut langs = self.languages();
        langs.sort();
        for lang in langs {
            let features = self.features_for(lang);
            let names: Vec<String> = features.iter().map(|f| f.to_string()).collect();
            out.push_str(&format!("{}: {}\n", lang, names.join(", ")));
        }
        out
    }
}

// ── ProviderChain ──

/// Entry in a [`ProviderChain`], pairing a registration with a priority.
#[derive(Debug, Clone, PartialEq)]
pub struct PrioritizedProvider {
    pub registration: ProviderRegistration,
    pub priority: ProviderPriority,
}

/// Chains multiple providers in priority order for a single feature.
///
/// When resolving which provider(s) to invoke, an `Exclusive` provider
/// suppresses all others. Otherwise providers are returned in descending
/// priority order (High before Default), with ties broken by insertion order.
#[derive(Debug, Clone, Default)]
pub struct ProviderChain {
    providers: Vec<PrioritizedProvider>,
}

impl ProviderChain {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a provider with the given priority.
    pub fn add(&mut self, registration: ProviderRegistration, priority: ProviderPriority) {
        self.providers.push(PrioritizedProvider { registration, priority });
    }

    /// Number of providers in the chain.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Returns `true` if the chain contains no providers.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Resolve the chain: if any provider is `Exclusive`, return only that one
    /// (the first exclusive provider wins). Otherwise return all providers
    /// sorted by descending priority weight.
    pub fn resolve(&self) -> Vec<&PrioritizedProvider> {
        // Check for an exclusive provider first.
        if let Some(exclusive) = self.providers.iter().find(|p| p.priority == ProviderPriority::Exclusive) {
            return vec![exclusive];
        }
        let mut sorted: Vec<&PrioritizedProvider> = self.providers.iter().collect();
        sorted.sort_by(|a, b| b.priority.weight().cmp(&a.priority.weight()));
        sorted
    }

    /// Convenience: resolve and return only the top provider, if any.
    pub fn top(&self) -> Option<&PrioritizedProvider> {
        self.resolve().into_iter().next()
    }

    /// Returns all provider IDs in resolved order.
    pub fn resolved_ids(&self) -> Vec<&str> {
        self.resolve()
            .iter()
            .map(|p| p.registration.provider_id.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// LanguageFeatureRegistry – track providers by language
// ---------------------------------------------------------------------------

/// Tracks which language feature providers are registered per language.
#[derive(Debug, Clone)]
pub struct LanguageFeatureRegistry {
    /// Map of language ID to registered feature kinds.
    features: HashMap<String, Vec<LanguageFeatureKind>>,
}

impl Default for LanguageFeatureRegistry {
    fn default() -> Self {
        Self {
            features: HashMap::new(),
        }
    }
}

impl LanguageFeatureRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a feature kind for a language.
    pub fn register(&mut self, language: &str, kind: LanguageFeatureKind) {
        self.features
            .entry(language.to_string())
            .or_default()
            .push(kind);
    }

    /// Check if a language has a specific feature.
    pub fn has_feature(&self, language: &str, kind: LanguageFeatureKind) -> bool {
        self.features
            .get(language)
            .map(|kinds| kinds.contains(&kind))
            .unwrap_or(false)
    }

    /// Get all features for a language.
    pub fn get_features(&self, language: &str) -> Vec<LanguageFeatureKind> {
        self.features.get(language).cloned().unwrap_or_default()
    }

    /// Get all languages that have at least one feature.
    pub fn languages(&self) -> Vec<&str> {
        self.features.keys().map(|s| s.as_str()).collect()
    }

    /// Total number of feature registrations across all languages.
    pub fn total_registrations(&self) -> usize {
        self.features.values().map(|v| v.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// LanguageSelectorMatcher – pattern/scheme matching
// ---------------------------------------------------------------------------

/// Extended matching for language selectors with glob support.
#[derive(Debug, Clone)]
pub struct LanguageSelectorMatcher {
    selector: LanguageSelector,
}

impl LanguageSelectorMatcher {
    /// Create a matcher for the given selector.
    pub fn new(selector: LanguageSelector) -> Self {
        Self { selector }
    }

    /// Check if a document matches this selector.
    ///
    /// Matches against language, scheme, and file pattern.
    pub fn matches(&self, language: &str, scheme: &str, file_path: &str) -> bool {
        if let Some(ref lang) = self.selector.language {
            if lang != language {
                return false;
            }
        }
        if let Some(ref s) = self.selector.scheme {
            if s != scheme {
                return false;
            }
        }
        if let Some(ref pattern) = self.selector.pattern {
            if !Self::glob_match(pattern, file_path) {
                return false;
            }
        }
        true
    }

    /// Simple glob matching supporting `*` and `**`.
    fn glob_match(pattern: &str, path: &str) -> bool {
        if pattern == "**" {
            return true;
        }
        if let Some(ext) = pattern.strip_prefix("**/*.") {
            return path.ends_with(&format!(".{}", ext));
        }
        if let Some(ext) = pattern.strip_prefix("*.") {
            let file_name = path.rsplit('/').next().unwrap_or(path);
            return file_name.ends_with(&format!(".{}", ext));
        }
        pattern == path
    }

    /// Compute a match score (higher is more specific).
    pub fn score(&self, language: &str, scheme: &str, file_path: &str) -> u32 {
        if !self.matches(language, scheme, file_path) {
            return 0;
        }
        let mut score = 0u32;
        if self.selector.language.is_some() {
            score += 10;
        }
        if self.selector.scheme.is_some() {
            score += 5;
        }
        if self.selector.pattern.is_some() {
            score += 3;
        }
        score
    }
}

// ---------------------------------------------------------------------------
// Semantic token legend management
// ---------------------------------------------------------------------------

/// Manages semantic token types and modifiers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTokenLegend {
    /// Token type names (e.g. "namespace", "type", "class").
    pub token_types: Vec<String>,
    /// Token modifier names (e.g. "declaration", "definition").
    pub token_modifiers: Vec<String>,
}

impl Default for SemanticTokenLegend {
    fn default() -> Self {
        Self {
            token_types: vec![
                "namespace".into(),
                "type".into(),
                "class".into(),
                "enum".into(),
                "interface".into(),
                "struct".into(),
                "typeParameter".into(),
                "parameter".into(),
                "variable".into(),
                "property".into(),
                "function".into(),
                "method".into(),
                "keyword".into(),
                "comment".into(),
                "string".into(),
                "number".into(),
                "operator".into(),
            ],
            token_modifiers: vec![
                "declaration".into(),
                "definition".into(),
                "readonly".into(),
                "static".into(),
                "deprecated".into(),
                "async".into(),
            ],
        }
    }
}

impl SemanticTokenLegend {
    /// Create an empty legend.
    pub fn empty() -> Self {
        Self {
            token_types: Vec::new(),
            token_modifiers: Vec::new(),
        }
    }

    /// Get the index for a token type, registering it if new.
    pub fn token_type_index(&mut self, name: &str) -> usize {
        if let Some(idx) = self.token_types.iter().position(|t| t == name) {
            idx
        } else {
            self.token_types.push(name.to_string());
            self.token_types.len() - 1
        }
    }

    /// Get the bitmask for a set of modifiers.
    pub fn modifier_bitmask(&self, modifiers: &[&str]) -> u32 {
        let mut mask = 0u32;
        for name in modifiers {
            if let Some(idx) = self.token_modifiers.iter().position(|m| m == name) {
                mask |= 1 << idx;
            }
        }
        mask
    }

    /// Whether a token type is registered.
    pub fn has_token_type(&self, name: &str) -> bool {
        self.token_types.iter().any(|t| t == name)
    }
}

// ---------------------------------------------------------------------------
// Language-specific settings overlay
// ---------------------------------------------------------------------------

/// Overlays language-specific settings on top of defaults.
#[derive(Debug, Clone)]
pub struct LanguageSettingsOverlay {
    overrides: HashMap<String, HashMap<String, String>>,
}

impl Default for LanguageSettingsOverlay {
    fn default() -> Self {
        Self {
            overrides: HashMap::new(),
        }
    }
}

impl LanguageSettingsOverlay {
    /// Create an empty overlay.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a language-specific setting.
    pub fn set(&mut self, language: &str, key: &str, value: &str) {
        self.overrides
            .entry(language.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
    }

    /// Get a setting value, with a fallback default.
    pub fn get<'a>(&'a self, language: &str, key: &str, default: &'a str) -> &'a str {
        self.overrides
            .get(language)
            .and_then(|m| m.get(key))
            .map(|s| s.as_str())
            .unwrap_or(default)
    }

    /// Get all overridden keys for a language.
    pub fn keys_for(&self, language: &str) -> Vec<&str> {
        self.overrides
            .get(language)
            .map(|m| m.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default()
    }

    /// Number of languages with overrides.
    pub fn language_count(&self) -> usize {
        self.overrides.len()
    }
}


// ---------------------------------------------------------------------------
// Language status bar integration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageStatusItem {
    pub id: String, pub language_id: String, pub label: String,
    pub detail: Option<String>, pub severity: LanguageStatusSeverity, pub busy: bool, pub command: Option<String>,
}
impl LanguageStatusItem {
    pub fn new(id: impl Into<String>, lang: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), language_id: lang.into(), label: label.into(), detail: None, severity: LanguageStatusSeverity::Information, busy: false, command: None }
    }
    pub fn with_detail(mut self, d: impl Into<String>) -> Self { self.detail = Some(d.into()); self }
    pub fn with_severity(mut self, s: LanguageStatusSeverity) -> Self { self.severity = s; self }
    pub fn with_busy(mut self, b: bool) -> Self { self.busy = b; self }
    pub fn is_error(&self) -> bool { self.severity == LanguageStatusSeverity::Error }
    pub fn is_warning(&self) -> bool { self.severity == LanguageStatusSeverity::Warning }
}
impl fmt::Display for LanguageStatusItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "[{}] {} ({})", self.severity, self.label, self.language_id) }
}

pub struct LanguageStatusBarService { items: Vec<LanguageStatusItem> }
impl LanguageStatusBarService {
    pub fn new() -> Self { Self { items: Vec::new() } }
    pub fn add_item(&mut self, item: LanguageStatusItem) { self.items.retain(|i| i.id != item.id); self.items.push(item); }
    pub fn remove_item(&mut self, id: &str) -> bool { let b = self.items.len(); self.items.retain(|i| i.id != id); self.items.len() < b }
    pub fn has_errors(&self) -> bool { self.items.iter().any(|i| i.is_error()) }
    pub fn has_warnings(&self) -> bool { self.items.iter().any(|i| i.is_warning()) }
    pub fn item_count(&self) -> usize { self.items.len() }
    pub fn busy_count(&self) -> usize { self.items.iter().filter(|i| i.busy).count() }
    pub fn clear(&mut self) { self.items.clear(); }
}
impl Default for LanguageStatusBarService { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// Language diagnostics summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageDiagnosticsSummary {
    pub language_id: String, pub error_count: usize, pub warning_count: usize,
    pub info_count: usize, pub hint_count: usize, pub file_count: usize,
}
impl LanguageDiagnosticsSummary {
    pub fn new(lang: impl Into<String>) -> Self { Self { language_id: lang.into(), ..Default::default() } }
    pub fn total(&self) -> usize { self.error_count + self.warning_count + self.info_count + self.hint_count }
    pub fn has_problems(&self) -> bool { self.error_count > 0 || self.warning_count > 0 }
    pub fn merge(&mut self, o: &Self) { self.error_count += o.error_count; self.warning_count += o.warning_count; self.info_count += o.info_count; self.hint_count += o.hint_count; self.file_count += o.file_count; }
    pub fn reset(&mut self) { self.error_count = 0; self.warning_count = 0; self.info_count = 0; self.hint_count = 0; self.file_count = 0; }
}
impl fmt::Display for LanguageDiagnosticsSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}: {} errors, {} warnings ({} files)", self.language_id, self.error_count, self.warning_count, self.file_count) }
}

pub struct DiagnosticsSummaryCollector { summaries: HashMap<String, LanguageDiagnosticsSummary> }
impl DiagnosticsSummaryCollector {
    pub fn new() -> Self { Self { summaries: HashMap::new() } }
    pub fn record_error(&mut self, l: &str) { self.summaries.entry(l.to_string()).or_insert_with(|| LanguageDiagnosticsSummary::new(l)).error_count += 1; }
    pub fn record_warning(&mut self, l: &str) { self.summaries.entry(l.to_string()).or_insert_with(|| LanguageDiagnosticsSummary::new(l)).warning_count += 1; }
    pub fn record_info(&mut self, l: &str) { self.summaries.entry(l.to_string()).or_insert_with(|| LanguageDiagnosticsSummary::new(l)).info_count += 1; }
    pub fn get_summary(&self, l: &str) -> Option<&LanguageDiagnosticsSummary> { self.summaries.get(l) }
    pub fn total_errors(&self) -> usize { self.summaries.values().map(|s| s.error_count).sum() }
    pub fn total_warnings(&self) -> usize { self.summaries.values().map(|s| s.warning_count).sum() }
    pub fn languages_with_errors(&self) -> Vec<&str> { self.summaries.values().filter(|s| s.error_count > 0).map(|s| s.language_id.as_str()).collect() }
    pub fn clear(&mut self) { self.summaries.clear(); }
    pub fn language_count(&self) -> usize { self.summaries.len() }
}
impl Default for DiagnosticsSummaryCollector { fn default() -> Self { Self::new() } }


// ---------------------------------------------------------------------------
// LanguageStatusItemConfig — configuration for LanguageStatusItem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LanguageStatusItemConfig {
    pub max_entries: usize,
    pub auto_refresh: bool,
    pub refresh_interval_ms: u64,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl LanguageStatusItemConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_refresh(mut self, a: bool) -> Self { self.auto_refresh = a; self }
    pub fn with_refresh_interval(mut self, ms: u64) -> Self { self.refresh_interval_ms = ms; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn is_refresh_due(&self, elapsed_ms: u64) -> bool { self.auto_refresh && elapsed_ms >= self.refresh_interval_ms }
}

impl Default for LanguageStatusItemConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_refresh: true, refresh_interval_ms: 5000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for LanguageStatusItemConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_refresh={}, interval={}ms)", self.max_entries, self.auto_refresh, self.refresh_interval_ms)
    }
}

// ---------------------------------------------------------------------------
// DiagnosticsSummaryCollectorStats — statistics tracker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct DiagnosticsSummaryCollectorStats {
    pub total_operations: u64,
    pub successful: u64,
    pub failed: u64,
    pub total_duration_ms: u64,
    pub peak_concurrent: usize,
    pub current_concurrent: usize,
}

impl DiagnosticsSummaryCollectorStats {
    pub fn new() -> Self { Self::default() }
    pub fn record_success(&mut self, duration_ms: u64) {
        self.total_operations += 1; self.successful += 1; self.total_duration_ms += duration_ms;
    }
    pub fn record_failure(&mut self, duration_ms: u64) {
        self.total_operations += 1; self.failed += 1; self.total_duration_ms += duration_ms;
    }
    pub fn success_rate(&self) -> f64 { if self.total_operations == 0 { 0.0 } else { self.successful as f64 / self.total_operations as f64 } }
    pub fn avg_duration_ms(&self) -> f64 { if self.total_operations == 0 { 0.0 } else { self.total_duration_ms as f64 / self.total_operations as f64 } }
    pub fn update_concurrent(&mut self, current: usize) {
        self.current_concurrent = current;
        if current > self.peak_concurrent { self.peak_concurrent = current; }
    }
    pub fn reset(&mut self) { *self = Self::default(); }
    pub fn merge(&mut self, other: &Self) {
        self.total_operations += other.total_operations;
        self.successful += other.successful;
        self.failed += other.failed;
        self.total_duration_ms += other.total_duration_ms;
        if other.peak_concurrent > self.peak_concurrent { self.peak_concurrent = other.peak_concurrent; }
    }
}

impl fmt::Display for DiagnosticsSummaryCollectorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(ops={}, success={:.1}%, avg={:.1}ms)", self.total_operations, self.success_rate() * 100.0, self.avg_duration_ms())
    }
}

// ---------------------------------------------------------------------------
// LanguageStatusItemEventKind — event types for LanguageStatusItem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageStatusItemEventKind {
    Created,
    Updated,
    Deleted,
    Refreshed,
    Error,
}

impl fmt::Display for LanguageStatusItemEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Updated => write!(f, "updated"),
            Self::Deleted => write!(f, "deleted"),
            Self::Refreshed => write!(f, "refreshed"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A recorded event in the LanguageStatusItem lifecycle.
#[derive(Debug, Clone)]
pub struct LanguageStatusItemEvent {
    pub kind: LanguageStatusItemEventKind,
    pub timestamp: u64,
    pub detail: Option<String>,
}

impl LanguageStatusItemEvent {
    pub fn new(kind: LanguageStatusItemEventKind, timestamp: u64) -> Self {
        Self { kind, timestamp, detail: None }
    }
    pub fn with_detail(mut self, d: impl Into<String>) -> Self { self.detail = Some(d.into()); self }
    pub fn is_error(&self) -> bool { self.kind == LanguageStatusItemEventKind::Error }
}

impl fmt::Display for LanguageStatusItemEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Event({}, t={})", self.kind, self.timestamp)
    }
}


// ---------------------------------------------------------------------------
// vsedit-ext-languages: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtLanguagesXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl ExtLanguagesXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for ExtLanguagesXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct ExtLanguagesXRegistry {
    entries: Vec<ExtLanguagesXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl ExtLanguagesXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: ExtLanguagesXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ExtLanguagesXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ExtLanguagesXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<ExtLanguagesXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&ExtLanguagesXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&ExtLanguagesXConfig> {
        let mut sorted: Vec<&ExtLanguagesXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&ExtLanguagesXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> ExtLanguagesXIterator<'_> {
        ExtLanguagesXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct ExtLanguagesXIterator<'a> {
    inner: std::slice::Iter<'a, ExtLanguagesXConfig>,
}

impl<'a> Iterator for ExtLanguagesXIterator<'a> {
    type Item = &'a ExtLanguagesXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct ExtLanguagesXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl ExtLanguagesXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct ExtLanguagesXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl ExtLanguagesXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &ExtLanguagesXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &ExtLanguagesXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &ExtLanguagesXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for ExtLanguagesXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct ExtLanguagesXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl ExtLanguagesXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &ExtLanguagesXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &ExtLanguagesXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for ExtLanguagesXValidator {
    fn default() -> Self {
        Self::new()
    }
}



// ---------------------------------------------------------------------------
// ext_languages – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for extension language contributions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExtLanguagesLanguageFeature {
    Completion,
    Diagnostics,
    Formatting,
    Hover,
}

impl YExtLanguagesLanguageFeature {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Completion => 0,
            Self::Diagnostics => 1,
            Self::Formatting => 2,
            Self::Hover => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Completion => "Completion",
            Self::Diagnostics => "Diagnostics",
            Self::Formatting => "Formatting",
            Self::Hover => "Hover",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExtLanguagesLanguageFeature] {
        &[
            YExtLanguagesLanguageFeature::Completion,
            YExtLanguagesLanguageFeature::Diagnostics,
            YExtLanguagesLanguageFeature::Formatting,
            YExtLanguagesLanguageFeature::Hover,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExtLanguagesLanguageFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks language registration data.
#[derive(Debug, Clone)]
pub struct YExtLanguagesLanguageRegistration {
    pub language_id: String,
    pub extensions: Vec<String>,
    pub aliases: Vec<String>,
}

impl YExtLanguagesLanguageRegistration {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            language_id: String::new(),
            extensions: Vec::new(),
            aliases: Vec::new(),
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.extensions.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.extensions.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExtLanguagesLanguageRegistration({}: {:?})", "language_id", self.language_id)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_ext_languages_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_ext_languages_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_ext_languages_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_ext_languages_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_ext_languages_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_ext_languages_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_ext_languages_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_ext_languages_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// ext_languages – Extended language detector helpers
// ---------------------------------------------------------------------------

/// Priority levels for language detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExtLanguagesPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExtLanguagesPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZExtLanguagesPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExtLanguagesPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks language detector data.
#[derive(Debug, Clone)]
pub struct ZExtLanguagesLanguageDetector {
    pub signatures: Vec<(String, Vec<String>)>,
    pub confidence_threshold: f64,
    pub cache_size: usize,
}

impl ZExtLanguagesLanguageDetector {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            signatures: Vec::new(),
            confidence_threshold: 0.0,
            cache_size: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.signatures.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.signatures.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.signatures.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExtLanguagesLanguageDetector[confidence_threshold={:?}, cache_size={:?}]", self.confidence_threshold, self.cache_size)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for language detector.
pub fn z_ext_languages_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_ext_languages_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_ext_languages_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_ext_languages_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_ext_languages_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_ext_languages_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_ext_languages_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
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

    // ── SelectorMatcher tests ──

    #[test]
    fn selector_matcher_matches_and_scores() {
        let matcher = SelectorMatcher::new("rust", "file", "/src/main.rs");
        let sel_match = LanguageSelectorBuilder::new().language("rust").scheme("file").build();
        let sel_miss = LanguageSelectorBuilder::new().language("python").build();

        assert!(matcher.matches(&sel_match));
        assert!(!matcher.matches(&sel_miss));
        assert!(matcher.score(&sel_match) > 0);
        assert_eq!(matcher.score(&sel_miss), 0);
    }

    #[test]
    fn selector_matcher_filter_and_best() {
        let matcher = SelectorMatcher::new("rust", "file", "/src/lib.rs");
        let selectors = vec![
            LanguageSelectorBuilder::new().build(),                              // wildcard
            LanguageSelectorBuilder::new().language("rust").build(),             // lang only
            LanguageSelectorBuilder::new().language("rust").scheme("file").pattern("**/*.rs").build(), // full
            LanguageSelectorBuilder::new().language("python").build(),           // no match
        ];
        let matching = matcher.filter_matching(&selectors);
        assert_eq!(matching.len(), 3);

        let best = matcher.best_match(&selectors).unwrap();
        assert_eq!(best.language.as_deref(), Some("rust"));
        assert_eq!(best.scheme.as_deref(), Some("file"));
        assert_eq!(best.pattern.as_deref(), Some("**/*.rs"));
    }

    // ── FeatureMatrix tests ──

    #[test]
    fn feature_matrix_add_and_query() {
        let mut matrix = FeatureMatrix::new();
        matrix.add("rust", LanguageFeatureKind::Completion);
        matrix.add("rust", LanguageFeatureKind::Hover);
        matrix.add("rust", LanguageFeatureKind::Completion); // duplicate, ignored
        matrix.add("python", LanguageFeatureKind::Formatting);

        assert_eq!(matrix.features_for("rust").len(), 2);
        assert_eq!(matrix.features_for("python").len(), 1);
        assert_eq!(matrix.features_for("go").len(), 0);
        assert_eq!(matrix.total_entries(), 3);
        assert_eq!(matrix.languages(), vec!["python", "rust"]);
    }

    #[test]
    fn feature_matrix_languages_with_all_and_any() {
        let mut matrix = FeatureMatrix::new();
        matrix.add("rust", LanguageFeatureKind::Completion);
        matrix.add("rust", LanguageFeatureKind::Hover);
        matrix.add("rust", LanguageFeatureKind::Formatting);
        matrix.add("python", LanguageFeatureKind::Completion);
        matrix.add("go", LanguageFeatureKind::Hover);

        let with_both = matrix.languages_with_all(&[
            LanguageFeatureKind::Completion,
            LanguageFeatureKind::Hover,
        ]);
        assert_eq!(with_both, vec!["rust"]);

        let mut with_any = matrix.languages_with_any(&[LanguageFeatureKind::Formatting]);
        with_any.sort();
        assert_eq!(with_any, vec!["rust"]);
    }

    #[test]
    fn feature_matrix_from_bridge() {
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
                selector: selector("go"),
            },
        });
        let matrix = FeatureMatrix::from_bridge(&bridge);
        assert_eq!(matrix.features_for("rust").len(), 2);
        assert_eq!(matrix.features_for("go").len(), 1);

        let summary = matrix.summary();
        assert!(summary.contains("rust:"));
        assert!(summary.contains("go:"));
    }

    // ── ProviderChain tests ──

    #[test]
    fn provider_chain_resolve_by_priority() {
        let mut chain = ProviderChain::new();
        assert!(chain.is_empty());

        chain.add(
            ProviderRegistration { provider_id: "default-1".into(), selector: selector("rust") },
            ProviderPriority::Default,
        );
        chain.add(
            ProviderRegistration { provider_id: "high-1".into(), selector: selector("rust") },
            ProviderPriority::High,
        );
        chain.add(
            ProviderRegistration { provider_id: "default-2".into(), selector: selector("rust") },
            ProviderPriority::Default,
        );
        assert_eq!(chain.len(), 3);

        let ids = chain.resolved_ids();
        assert_eq!(ids[0], "high-1");
        // The two default providers follow
        assert!(ids.contains(&"default-1"));
        assert!(ids.contains(&"default-2"));
    }

    #[test]
    fn provider_chain_exclusive_suppresses_others() {
        let mut chain = ProviderChain::new();
        chain.add(
            ProviderRegistration { provider_id: "default-1".into(), selector: selector("rust") },
            ProviderPriority::Default,
        );
        chain.add(
            ProviderRegistration { provider_id: "excl-1".into(), selector: selector("rust") },
            ProviderPriority::Exclusive,
        );
        chain.add(
            ProviderRegistration { provider_id: "high-1".into(), selector: selector("rust") },
            ProviderPriority::High,
        );

        let resolved = chain.resolve();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].registration.provider_id, "excl-1");
        assert_eq!(chain.top().unwrap().registration.provider_id, "excl-1");
    }

    // ── Display / From impl tests ──

    #[test]
    fn display_impls_produce_output() {
        assert_eq!(LanguageFeatureKind::Completion.to_string(), "Completion");
        assert_eq!(LanguageFeatureKind::SignatureHelp.to_string(), "Signature Help");
        assert_eq!(ProviderPriority::High.to_string(), "high");
        assert_eq!(LanguageStatusSeverity::Error.to_string(), "error");

        let sel = LanguageSelectorBuilder::new().language("rust").scheme("file").build();
        assert_eq!(sel.to_string(), "rust:file:*");

        let status = LanguageStatus::new("rust", "RA").with_detail("ready");
        let display = status.to_string();
        assert!(display.contains("RA (ready)"));
        assert!(display.contains("rust"));
    }

    #[test]
    fn from_str_for_selector() {
        let sel: LanguageSelector = "typescript".into();
        assert_eq!(sel.language.as_deref(), Some("typescript"));
        assert!(sel.scheme.is_none());
        assert!(sel.pattern.is_none());
    }

    #[test]
    fn from_priority_to_u32() {
        let w: u32 = ProviderPriority::Exclusive.into();
        assert_eq!(w, 2);
        let w2: u32 = ProviderPriority::Default.into();
        assert_eq!(w2, 0);
    }

    // -- LanguageFeatureRegistry tests --

    #[test]
    fn feature_registry_register_and_check() {
        let mut reg = LanguageFeatureRegistry::new();
        reg.register("rust", LanguageFeatureKind::Completion);
        reg.register("rust", LanguageFeatureKind::Hover);
        assert!(reg.has_feature("rust", LanguageFeatureKind::Completion));
        assert!(!reg.has_feature("rust", LanguageFeatureKind::Definition));
        assert!(!reg.has_feature("python", LanguageFeatureKind::Completion));
    }

    #[test]
    fn feature_registry_languages() {
        let mut reg = LanguageFeatureRegistry::new();
        reg.register("rust", LanguageFeatureKind::Completion);
        reg.register("python", LanguageFeatureKind::Hover);
        let langs = reg.languages();
        assert_eq!(langs.len(), 2);
    }

    #[test]
    fn feature_registry_total() {
        let mut reg = LanguageFeatureRegistry::new();
        reg.register("rust", LanguageFeatureKind::Completion);
        reg.register("rust", LanguageFeatureKind::Hover);
        assert_eq!(reg.total_registrations(), 2);
    }

    // -- LanguageSelectorMatcher tests --

    #[test]
    fn selector_matcher_language_only() {
        let sel = LanguageSelector { language: Some("rust".into()), scheme: None, pattern: None };
        let m = LanguageSelectorMatcher::new(sel);
        assert!(m.matches("rust", "file", "main.rs"));
        assert!(!m.matches("python", "file", "main.py"));
    }

    #[test]
    fn selector_matcher_pattern() {
        let sel = LanguageSelector {
            language: None,
            scheme: None,
            pattern: Some("**/*.rs".into()),
        };
        let m = LanguageSelectorMatcher::new(sel);
        assert!(m.matches("rust", "file", "src/main.rs"));
        assert!(!m.matches("rust", "file", "src/main.py"));
    }

    #[test]
    fn selector_matcher_score() {
        let sel = LanguageSelector {
            language: Some("rust".into()),
            scheme: Some("file".into()),
            pattern: None,
        };
        let m = LanguageSelectorMatcher::new(sel);
        assert_eq!(m.score("rust", "file", "main.rs"), 15);
        assert_eq!(m.score("python", "file", "main.py"), 0);
    }

    // -- SemanticTokenLegend tests --

    #[test]
    fn legend_default_has_types() {
        let leg = SemanticTokenLegend::default();
        assert!(leg.has_token_type("function"));
        assert!(leg.has_token_type("variable"));
    }

    #[test]
    fn legend_token_type_index() {
        let mut leg = SemanticTokenLegend::empty();
        let idx = leg.token_type_index("custom");
        assert_eq!(idx, 0);
        let idx2 = leg.token_type_index("custom");
        assert_eq!(idx2, 0);
    }

    #[test]
    fn legend_modifier_bitmask() {
        let leg = SemanticTokenLegend::default();
        let mask = leg.modifier_bitmask(&["declaration", "readonly"]);
        assert_eq!(mask, 0b101);
    }

    // -- LanguageSettingsOverlay tests --

    #[test]
    fn settings_overlay_set_get() {
        let mut o = LanguageSettingsOverlay::new();
        o.set("rust", "tabSize", "4");
        assert_eq!(o.get("rust", "tabSize", "2"), "4");
        assert_eq!(o.get("rust", "insertSpaces", "true"), "true");
        assert_eq!(o.get("python", "tabSize", "2"), "2");
    }

    #[test]
    fn settings_overlay_keys() {
        let mut o = LanguageSettingsOverlay::new();
        o.set("rust", "tabSize", "4");
        o.set("rust", "formatOnSave", "true");
        let keys = o.keys_for("rust");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn settings_overlay_language_count() {
        let mut o = LanguageSettingsOverlay::new();
        o.set("rust", "tabSize", "4");
        o.set("python", "tabSize", "4");
        assert_eq!(o.language_count(), 2);
    }

    #[test] fn lang_status_create() { let i = LanguageStatusItem::new("a","rust","R"); assert_eq!(i.language_id, "rust"); }
    #[test] fn lang_status_detail() { let i = LanguageStatusItem::new("a","r","R").with_detail("d"); assert_eq!(i.detail.unwrap(), "d"); }
    #[test] fn lang_sev_display() { assert_eq!(format!("{}", LanguageStatusSeverity::Error), "error"); }
    #[test] fn lang_bar_add_rm() { let mut s = LanguageStatusBarService::new(); s.add_item(LanguageStatusItem::new("a","r","t")); assert_eq!(s.item_count(), 1); s.remove_item("a"); assert_eq!(s.item_count(), 0); }
    #[test] fn lang_bar_dedup() { let mut s = LanguageStatusBarService::new(); s.add_item(LanguageStatusItem::new("a","r","v1")); s.add_item(LanguageStatusItem::new("a","r","v2")); assert_eq!(s.item_count(), 1); }
    #[test] fn lang_bar_errors() { let mut s = LanguageStatusBarService::new(); s.add_item(LanguageStatusItem::new("a","r","e").with_severity(LanguageStatusSeverity::Error)); assert!(s.has_errors()); }
    #[test] fn diag_sum_total() { let mut s = LanguageDiagnosticsSummary::new("r"); s.error_count = 3; s.warning_count = 2; s.info_count = 1; assert_eq!(s.total(), 6); }
    #[test] fn diag_sum_merge() { let mut a = LanguageDiagnosticsSummary::new("r"); a.error_count = 1; let mut b = LanguageDiagnosticsSummary::new("r"); b.error_count = 2; a.merge(&b); assert_eq!(a.error_count, 3); }
    #[test] fn diag_collector_rec() { let mut c = DiagnosticsSummaryCollector::new(); c.record_error("r"); c.record_error("r"); c.record_warning("p"); assert_eq!(c.total_errors(), 2); assert_eq!(c.total_warnings(), 1); }
    #[test] fn diag_collector_langs() { let mut c = DiagnosticsSummaryCollector::new(); c.record_error("r"); c.record_warning("p"); assert_eq!(c.languages_with_errors().len(), 1); }
    #[test] fn diag_sum_display() { let mut s = LanguageDiagnosticsSummary::new("r"); s.error_count = 5; assert!(format!("{}", s).contains("5 errors")); }
    #[test] fn lang_status_display() { assert!(format!("{}", LanguageStatusItem::new("a","rust","R")).contains("rust")); }


    #[test] fn languageStatusItem_cfg_default() {
        let c = LanguageStatusItemConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_refresh);
    }
    #[test] fn languageStatusItem_cfg_builder() {
        let c = LanguageStatusItemConfig::new().with_max_entries(500).with_auto_refresh(false);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_refresh);
    }
    #[test] fn languageStatusItem_cfg_labels() {
        let mut c = LanguageStatusItemConfig::new();
        c.set_label("x", "y");
        assert_eq!(c.get_label("x"), Some("y"));
    }
    #[test] fn languageStatusItem_cfg_refresh_due() {
        let c = LanguageStatusItemConfig::new();
        assert!(!c.is_refresh_due(1000));
        assert!(c.is_refresh_due(6000));
    }
    #[test] fn languageStatusItem_cfg_display() {
        assert!(format!("{}", LanguageStatusItemConfig::new()).contains("Config"));
    }
    #[test] fn diagnosticsSummaryCollector_stats_success() {
        let mut st = DiagnosticsSummaryCollectorStats::new();
        st.record_success(10);
        st.record_success(20);
        st.record_failure(5);
        assert_eq!(st.total_operations, 3);
        assert!((st.success_rate() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn diagnosticsSummaryCollector_stats_avg_dur() {
        let mut st = DiagnosticsSummaryCollectorStats::new();
        st.record_success(10);
        st.record_success(30);
        assert!((st.avg_duration_ms() - 20.0).abs() < 1e-9);
    }
    #[test] fn diagnosticsSummaryCollector_stats_merge() {
        let mut a = DiagnosticsSummaryCollectorStats::new();
        a.record_success(10);
        let mut b = DiagnosticsSummaryCollectorStats::new();
        b.record_success(20);
        a.merge(&b);
        assert_eq!(a.total_operations, 2);
    }
    #[test] fn diagnosticsSummaryCollector_stats_concurrent() {
        let mut st = DiagnosticsSummaryCollectorStats::new();
        st.update_concurrent(5);
        st.update_concurrent(3);
        assert_eq!(st.peak_concurrent, 5);
    }
    #[test] fn diagnosticsSummaryCollector_stats_display() {
        assert!(format!("{}", DiagnosticsSummaryCollectorStats::new()).contains("Stats"));
    }
    #[test] fn languageStatusItem_event_new() {
        let e = LanguageStatusItemEvent::new(LanguageStatusItemEventKind::Created, 100);
        assert_eq!(e.kind, LanguageStatusItemEventKind::Created);
        assert!(!e.is_error());
    }
    #[test] fn languageStatusItem_event_detail() {
        let e = LanguageStatusItemEvent::new(LanguageStatusItemEventKind::Error, 0).with_detail("oops");
        assert!(e.is_error());
        assert_eq!(e.detail.unwrap(), "oops");
    }
    #[test] fn languageStatusItem_event_display() {
        let e = LanguageStatusItemEvent::new(LanguageStatusItemEventKind::Updated, 50);
        assert!(format!("{}", e).contains("updated"));
    }
    #[test] fn languageStatusItem_event_kind_display() {
        assert_eq!(format!("{}", LanguageStatusItemEventKind::Refreshed), "refreshed");
    }


    #[test]
    fn extLanguages_x_config_new() {
        let c = ExtLanguagesXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn extLanguages_x_config_builder() {
        let c = ExtLanguagesXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn extLanguages_x_config_display() {
        let c = ExtLanguagesXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn extLanguages_x_registry_insert_get() {
        let mut reg = ExtLanguagesXRegistry::new();
        reg.insert(ExtLanguagesXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extLanguages_x_registry_duplicate() {
        let mut reg = ExtLanguagesXRegistry::new();
        reg.insert(ExtLanguagesXConfig::new("a")).unwrap();
        assert!(reg.insert(ExtLanguagesXConfig::new("a")).is_err());
    }

    #[test]
    fn extLanguages_x_registry_remove() {
        let mut reg = ExtLanguagesXRegistry::new();
        reg.insert(ExtLanguagesXConfig::new("a")).unwrap();
        reg.insert(ExtLanguagesXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn extLanguages_x_registry_active_entries() {
        let mut reg = ExtLanguagesXRegistry::new();
        reg.insert(ExtLanguagesXConfig::new("a")).unwrap();
        reg.insert(ExtLanguagesXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn extLanguages_x_registry_by_weight() {
        let mut reg = ExtLanguagesXRegistry::new();
        reg.insert(ExtLanguagesXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(ExtLanguagesXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn extLanguages_x_registry_tags() {
        let mut reg = ExtLanguagesXRegistry::new();
        reg.insert(ExtLanguagesXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(ExtLanguagesXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn extLanguages_x_registry_total_weight() {
        let mut reg = ExtLanguagesXRegistry::new();
        reg.insert(ExtLanguagesXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(ExtLanguagesXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn extLanguages_x_registry_iterator() {
        let mut reg = ExtLanguagesXRegistry::new();
        reg.insert(ExtLanguagesXConfig::new("a")).unwrap();
        reg.insert(ExtLanguagesXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn extLanguages_x_cache_put_get() {
        let mut cache = ExtLanguagesXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn extLanguages_x_cache_eviction() {
        let mut cache = ExtLanguagesXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn extLanguages_x_cache_lru_order() {
        let mut cache = ExtLanguagesXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn extLanguages_x_cache_most_least_recent() {
        let mut cache = ExtLanguagesXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn extLanguages_x_formatter_entry() {
        let e = ExtLanguagesXConfig::new("k").with_value("v");
        let fmt = ExtLanguagesXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn extLanguages_x_formatter_summary() {
        let mut reg = ExtLanguagesXRegistry::new();
        reg.insert(ExtLanguagesXConfig::new("a").with_weight(5)).unwrap();
        let fmt = ExtLanguagesXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn extLanguages_x_validator_valid() {
        let v = ExtLanguagesXValidator::new();
        let c = ExtLanguagesXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn extLanguages_x_validator_empty_key() {
        let v = ExtLanguagesXValidator::new();
        let c = ExtLanguagesXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extLanguages_x_validator_require_value() {
        let v = ExtLanguagesXValidator::new().require_value(true);
        let c = ExtLanguagesXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extLanguages_x_validator_allowed_tags() {
        let v = ExtLanguagesXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = ExtLanguagesXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn extLanguages_x_validator_validate_all() {
        let v = ExtLanguagesXValidator::new();
        let mut reg = ExtLanguagesXRegistry::new();
        reg.insert(ExtLanguagesXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    // -- ext_languages extended domain tests ----------------------------------------

    #[test]
    fn y_ext_languages_enum_index() {
        assert_eq!(YExtLanguagesLanguageFeature::Completion.index(), 0);
        assert_eq!(YExtLanguagesLanguageFeature::Diagnostics.index(), 1);
        assert_eq!(YExtLanguagesLanguageFeature::Formatting.index(), 2);
        assert_eq!(YExtLanguagesLanguageFeature::Hover.index(), 3);
    }

    #[test]
    fn y_ext_languages_enum_label() {
        assert_eq!(YExtLanguagesLanguageFeature::Completion.label(), "Completion");
        assert_eq!(YExtLanguagesLanguageFeature::Diagnostics.label(), "Diagnostics");
        assert_eq!(YExtLanguagesLanguageFeature::Formatting.label(), "Formatting");
        assert_eq!(YExtLanguagesLanguageFeature::Hover.label(), "Hover");
    }

    #[test]
    fn y_ext_languages_enum_all() {
        let all = YExtLanguagesLanguageFeature::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_ext_languages_enum_is_default() {
        assert!(YExtLanguagesLanguageFeature::Completion.is_default());
        assert!(!YExtLanguagesLanguageFeature::Hover.is_default());
    }

    #[test]
    fn y_ext_languages_enum_display() {
        assert_eq!(format!("{}", YExtLanguagesLanguageFeature::Completion), "Completion");
    }

    #[test]
    fn y_ext_languages_struct_new() {
        let s = YExtLanguagesLanguageRegistration::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_ext_languages_struct_clear() {
        let mut s = YExtLanguagesLanguageRegistration::new();
        s.extensions.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_ext_languages_fingerprint_deterministic() {
        let h1 = y_ext_languages_fingerprint("hello");
        let h2 = y_ext_languages_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_ext_languages_fingerprint("a"), y_ext_languages_fingerprint("b"));
    }

    #[test]
    fn y_ext_languages_truncate_short() {
        assert_eq!(y_ext_languages_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_ext_languages_truncate_long() {
        let r = y_ext_languages_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_ext_languages_normalize_key_basic() {
        assert_eq!(y_ext_languages_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_ext_languages_split_path_basic() {
        let parts = y_ext_languages_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_ext_languages_count_occurrences_basic() {
        assert_eq!(y_ext_languages_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_ext_languages_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_ext_languages_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_ext_languages_in_range_basic() {
        assert!(y_ext_languages_in_range(5, 1, 10));
        assert!(y_ext_languages_in_range(1, 1, 10));
        assert!(y_ext_languages_in_range(10, 1, 10));
        assert!(!y_ext_languages_in_range(0, 1, 10));
        assert!(!y_ext_languages_in_range(11, 1, 10));
    }

    #[test]
    fn y_ext_languages_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_ext_languages_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_ext_languages_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_ext_languages_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- ext_languages Z-extended tests -----------------------------------------------

    #[test]
    fn z_ext_languages_priority_weight() {
        assert_eq!(ZExtLanguagesPriority::Idle.weight(), 0);
        assert_eq!(ZExtLanguagesPriority::Normal.weight(), 2);
        assert_eq!(ZExtLanguagesPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_ext_languages_priority_label() {
        assert_eq!(ZExtLanguagesPriority::Low.label(), "low");
        assert_eq!(ZExtLanguagesPriority::High.label(), "high");
    }

    #[test]
    fn z_ext_languages_priority_is_elevated() {
        assert!(!ZExtLanguagesPriority::Normal.is_elevated());
        assert!(ZExtLanguagesPriority::High.is_elevated());
        assert!(ZExtLanguagesPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_ext_languages_priority_display() {
        assert_eq!(format!("{}", ZExtLanguagesPriority::Idle), "idle");
    }

    #[test]
    fn z_ext_languages_priority_all_asc() {
        let all = ZExtLanguagesPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExtLanguagesPriority::Idle);
        assert_eq!(all[4], ZExtLanguagesPriority::Realtime);
    }

    #[test]
    fn z_ext_languages_struct_new() {
        let s = ZExtLanguagesLanguageDetector::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_ext_languages_struct_toggled_clone() {
        let s = ZExtLanguagesLanguageDetector::new();
        let t = s.toggled_clone();
        let _ = t.cache_size;
    }

    #[test]
    fn z_ext_languages_rolling_hash_deterministic() {
        let h1 = z_ext_languages_rolling_hash(b"test");
        let h2 = z_ext_languages_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_ext_languages_rolling_hash(b"a"), z_ext_languages_rolling_hash(b"b"));
    }

    #[test]
    fn z_ext_languages_pad_to_basic() {
        assert_eq!(z_ext_languages_pad_to("hi", 5), "hi   ");
        assert_eq!(z_ext_languages_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_ext_languages_is_identifier_basic() {
        assert!(z_ext_languages_is_identifier("foo_bar"));
        assert!(z_ext_languages_is_identifier("abc123"));
        assert!(!z_ext_languages_is_identifier(""));
        assert!(!z_ext_languages_is_identifier("has space"));
    }

    #[test]
    fn z_ext_languages_levenshtein_basic() {
        assert_eq!(z_ext_languages_levenshtein("", ""), 0);
        assert_eq!(z_ext_languages_levenshtein("abc", "abc"), 0);
        assert_eq!(z_ext_languages_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_ext_languages_unique_words_basic() {
        let w = z_ext_languages_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_ext_languages_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_ext_languages_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_ext_languages_common_prefix_basic() {
        assert_eq!(z_ext_languages_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_ext_languages_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_ext_languages_struct_clear() {
        let mut s = ZExtLanguagesLanguageDetector::new();
        s.signatures.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_ext_languages_rolling_hash_empty() {
        let h = z_ext_languages_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }
}
