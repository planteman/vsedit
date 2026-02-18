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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 61
// ---------------------------------------------------------------------------

/// Generic object pool `Xc61Pool<T>`.
pub struct Xc61Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc61Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc61PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc61Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc61PoolStats {
        Xc61PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc61Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc61Scheduler`.
pub struct Xc61Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc61Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc61Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_61 hash for the given byte slice.
pub fn xc_61_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_61 convention.
pub fn xc_61_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_27 deepening: state machine + event bus ---

/// States for the Xd27 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd27State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd27State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd27Transition {
    pub from: Xd27State,
    pub to: Xd27State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd27StateMachine {
    current: Xd27State,
    history: Vec<Xd27Transition>,
    step_counter: usize,
}

impl Xd27StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd27State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd27State {
        self.current
    }

    pub fn history(&self) -> &[Xd27Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd27State) -> Result<Xd27State, String> {
        let allowed = match (self.current, target) {
            (Xd27State::Idle, Xd27State::Running) => true,
            (Xd27State::Running, Xd27State::Paused) => true,
            (Xd27State::Running, Xd27State::Done) => true,
            (Xd27State::Paused, Xd27State::Running) => true,
            (Xd27State::Paused, Xd27State::Done) => true,
            (Xd27State::Done, Xd27State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_27: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd27Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd27SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd27State> {
        let prefix = "Xd27SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd27State::Idle),
            "Running" => Some(Xd27State::Running),
            "Paused" => Some(Xd27State::Paused),
            "Done" => Some(Xd27State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd27State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd27 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd27Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd27Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd27HandlerFn = Box<dyn Fn(&Xd27Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd27EventBus {
    handlers: Vec<(usize, Option<String>, Xd27HandlerFn)>,
    next_id: usize,
    published: Vec<Xd27Event>,
}

impl Xd27EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd27Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd27Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd27Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd27Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #25
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf25Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf25TrieNode {
    children: std::collections::HashMap<char, Xf25TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf25Trie {
    root: Xf25TrieNode,
    count: usize,
}

impl Xf25Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf25TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf25TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf25TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf25BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf25BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 60).
pub struct Xh60SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh60SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 102 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 60).
pub struct Xh60BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh60BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 60).
pub struct Xi60Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi60Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi60Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi60Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 60).
pub struct Xi60IntervalTree {
    xi_intervals: Vec<Xi60Interval>,
}

impl Xi60IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi60Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi60Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi60Interval) -> Vec<&Xi60Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi60Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi60Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi60Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi60Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi60Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi60Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 61) ---

/// Disjoint set / union-find for crate 61.
pub struct Xj61UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj61UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ61_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 61.
pub struct Xj61BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj61BTreeNode<K, V>>>,
    len: usize,
}

struct Xj61BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj61BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj61BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ61_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ61_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj61BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj61BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj61BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj61BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_60 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk60SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk60SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk60DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk60DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
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

    // ---- xc_ pool / scheduler tests – block 61 ----

    #[test]
    fn xc_61_pool_new_empty() {
        let pool: super::Xc61Pool<i32> = super::Xc61Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_61_pool_release_acquire() {
        let mut pool = super::Xc61Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_61_pool_acquire_empty() {
        let mut pool: super::Xc61Pool<i32> = super::Xc61Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_61_pool_full() {
        let mut pool = super::Xc61Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_61_pool_drain() {
        let mut pool = super::Xc61Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_61_pool_stats() {
        let mut pool = super::Xc61Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_61_pool_clear() {
        let mut pool = super::Xc61Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_61_pool_shrink() {
        let mut pool = super::Xc61Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_61_pool_default() {
        let pool: super::Xc61Pool<String> = super::Xc61Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_61_pool_extend() {
        let mut pool = super::Xc61Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_61_pool_retain() {
        let mut pool = super::Xc61Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_61_scheduler_round_robin() {
        let mut sched = super::Xc61Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_61_scheduler_empty() {
        let mut sched = super::Xc61Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_61_scheduler_reset() {
        let mut sched = super::Xc61Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_61_scheduler_add_remove() {
        let mut sched = super::Xc61Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_61_scheduler_targets() {
        let sched = super::Xc61Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_61_hash_empty() {
        assert_eq!(super::xc_61_hash(b""), 5381);
    }

    #[test]
    fn xc_61_hash_data() {
        let h = super::xc_61_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_61_hash(b"hello"), h);
    }

    #[test]
    fn xc_61_reverse_str() {
        assert_eq!(super::xc_61_reverse("abc"), "cba");
        assert_eq!(super::xc_61_reverse(""), "");
    }


    // --- xd_27 deepening tests ---

    #[test]
    fn xd_27_sm_initial_state() {
        let sm = Xd27StateMachine::new();
        assert_eq!(sm.current_state(), Xd27State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_27_sm_valid_idle_to_running() {
        let mut sm = Xd27StateMachine::new();
        assert!(sm.transition(Xd27State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd27State::Running);
    }

    #[test]
    fn xd_27_sm_valid_running_to_paused() {
        let mut sm = Xd27StateMachine::new();
        sm.transition(Xd27State::Running).unwrap();
        assert!(sm.transition(Xd27State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd27State::Paused);
    }

    #[test]
    fn xd_27_sm_valid_running_to_done() {
        let mut sm = Xd27StateMachine::new();
        sm.transition(Xd27State::Running).unwrap();
        assert!(sm.transition(Xd27State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd27State::Done);
    }

    #[test]
    fn xd_27_sm_valid_paused_to_running() {
        let mut sm = Xd27StateMachine::new();
        sm.transition(Xd27State::Running).unwrap();
        sm.transition(Xd27State::Paused).unwrap();
        assert!(sm.transition(Xd27State::Running).is_ok());
    }

    #[test]
    fn xd_27_sm_valid_done_to_idle() {
        let mut sm = Xd27StateMachine::new();
        sm.transition(Xd27State::Running).unwrap();
        sm.transition(Xd27State::Done).unwrap();
        assert!(sm.transition(Xd27State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd27State::Idle);
    }

    #[test]
    fn xd_27_sm_invalid_idle_to_done() {
        let mut sm = Xd27StateMachine::new();
        assert!(sm.transition(Xd27State::Done).is_err());
    }

    #[test]
    fn xd_27_sm_invalid_idle_to_paused() {
        let mut sm = Xd27StateMachine::new();
        assert!(sm.transition(Xd27State::Paused).is_err());
    }

    #[test]
    fn xd_27_sm_history_tracking() {
        let mut sm = Xd27StateMachine::new();
        sm.transition(Xd27State::Running).unwrap();
        sm.transition(Xd27State::Paused).unwrap();
        sm.transition(Xd27State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd27State::Idle);
        assert_eq!(sm.history()[0].to, Xd27State::Running);
        assert_eq!(sm.history()[1].from, Xd27State::Running);
        assert_eq!(sm.history()[2].to, Xd27State::Done);
    }

    #[test]
    fn xd_27_sm_serialize_deserialize() {
        let mut sm = Xd27StateMachine::new();
        sm.transition(Xd27State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd27StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd27State::Running));
    }

    #[test]
    fn xd_27_sm_deserialize_invalid() {
        assert_eq!(Xd27StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_27_sm_reset() {
        let mut sm = Xd27StateMachine::new();
        sm.transition(Xd27State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd27State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_27_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd27EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd27Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_27_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd27EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd27Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd27Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_27_bus_unsubscribe() {
        let mut bus = Xd27EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_27_event_kind_and_payload() {
        let e = Xd27Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd27Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_27_bus_clear_history() {
        let mut bus = Xd27EventBus::new();
        bus.publish(Xd27Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_27_sm_step_counter_increments() {
        let mut sm = Xd27StateMachine::new();
        sm.transition(Xd27State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd27State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #25 --

    #[test]
    fn xf25_trie_insert_search() {
        let mut t = Xf25Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf25_trie_starts_with() {
        let mut t = Xf25Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf25_trie_remove() {
        let mut t = Xf25Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf25_trie_word_count() {
        let mut t = Xf25Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf25_trie_longest_prefix() {
        let mut t = Xf25Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf25_trie_all_words() {
        let mut t = Xf25Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf25_trie_autocomplete() {
        let mut t = Xf25Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf25_trie_empty_search() {
        let t = Xf25Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf25_bloom_add_contains() {
        let mut bf = Xf25BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf25_bloom_probably_absent() {
        let bf = Xf25BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf25_bloom_false_positive_rate() {
        let mut bf = Xf25BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf25_bloom_clear() {
        let mut bf = Xf25BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf25_bloom_union() {
        let mut a = Xf25BloomFilter::xf_new(512, 2);
        let mut b = Xf25BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf25_bloom_intersection_estimate() {
        let mut a = Xf25BloomFilter::xf_new(512, 2);
        let mut b = Xf25BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf25_bloom_union_size_mismatch() {
        let a = Xf25BloomFilter::xf_new(256, 2);
        let b = Xf25BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh60_skip_insert_contains() {
        let mut sl = super::Xh60SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh60_skip_remove() {
        let mut sl = super::Xh60SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh60_skip_len() {
        let mut sl = super::Xh60SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh60_skip_range_query() {
        let mut sl = super::Xh60SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh60_skip_floor_ceiling() {
        let mut sl = super::Xh60SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh60_skip_rank() {
        let mut sl = super::Xh60SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh60_skip_empty() {
        let sl = super::Xh60SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh60_skip_duplicates() {
        let mut sl = super::Xh60SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh60_bitset_set_test() {
        let mut bs = super::Xh60BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh60_bitset_clear_count() {
        let mut bs = super::Xh60BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh60_bitset_and_or_xor() {
        let mut a = super::Xh60BitSet::xh_new(128);
        let mut b = super::Xh60BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh60_bitset_iter_ones() {
        let mut bs = super::Xh60BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh60_bitset_first_last() {
        let mut bs = super::Xh60BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh60_bitset_empty() {
        let bs = super::Xh60BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi60_deque_push_pop_back() {
        let mut dq = super::Xi60Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi60_deque_push_pop_front() {
        let mut dq = super::Xi60Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi60_deque_mixed_ops() {
        let mut dq = super::Xi60Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi60_deque_get_and_split() {
        let mut dq = super::Xi60Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi60_deque_rotate_left() {
        let mut dq = super::Xi60Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi60_deque_rotate_right() {
        let mut dq = super::Xi60Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi60_deque_grow() {
        let mut dq = super::Xi60Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi60_deque_empty() {
        let dq = super::Xi60Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi60_interval_tree_insert_query() {
        let mut tree = super::Xi60IntervalTree::xi_new();
        tree.xi_insert(super::Xi60Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi60Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi60Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi60_interval_tree_overlap() {
        let mut tree = super::Xi60IntervalTree::xi_new();
        tree.xi_insert(super::Xi60Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi60Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi60Interval::xi_new(12, 20));
        let q = super::Xi60Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi60_interval_tree_remove() {
        let mut tree = super::Xi60IntervalTree::xi_new();
        tree.xi_insert(super::Xi60Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi60Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi60_interval_tree_gaps() {
        let mut tree = super::Xi60IntervalTree::xi_new();
        tree.xi_insert(super::Xi60Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi60Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi60Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi60Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi60Interval::xi_new(8, 10));
    }

    #[test]
    fn xi60_interval_tree_merge() {
        let mut tree = super::Xi60IntervalTree::xi_new();
        tree.xi_insert(super::Xi60Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi60Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi60Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi60Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi60Interval::xi_new(10, 15));
    }

    #[test]
    fn xi60_interval_tree_all() {
        let mut tree = super::Xi60IntervalTree::xi_new();
        tree.xi_insert(super::Xi60Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi60Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi60_interval_tree_empty() {
        let tree = super::Xi60IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi60_interval_tree_contains_point() {
        let iv = super::Xi60Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 61) ---

    #[test]
    fn xj_61_uf_make_and_find() {
        let mut uf = super::Xj61UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_61_uf_union_connected() {
        let mut uf = super::Xj61UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_61_uf_component_count() {
        let mut uf = super::Xj61UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_61_uf_component_size() {
        let mut uf = super::Xj61UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_61_uf_largest_component() {
        let mut uf = super::Xj61UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_61_uf_many_elements() {
        let mut uf = super::Xj61UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_61_uf_separate_components() {
        let mut uf = super::Xj61UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_61_uf_path_compression() {
        let mut uf = super::Xj61UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_61_bt_insert_get() {
        let mut bt = super::Xj61BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_61_bt_contains_len() {
        let mut bt = super::Xj61BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_61_bt_replace() {
        let mut bt = super::Xj61BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_61_bt_remove() {
        let mut bt = super::Xj61BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_61_bt_keys_values() {
        let mut bt = super::Xj61BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_61_bt_range() {
        let mut bt = super::Xj61BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_61_bt_min_max() {
        let mut bt = super::Xj61BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_61_bt_many_inserts() {
        let mut bt = super::Xj61BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_60 segment tree tests ---

    #[test]
    fn xk_60_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk60SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_60_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk60SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_60_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk60SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_60_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk60SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_60_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk60SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_60_st_single_element() {
        let data = vec![42];
        let st = super::Xk60SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_60_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk60SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_60_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk60SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_60 disjoint intervals tests ---

    #[test]
    fn xk_60_di_add_and_count() {
        let mut di = super::Xk60DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_60_di_merge_overlap() {
        let mut di = super::Xk60DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_60_di_contains() {
        let mut di = super::Xk60DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_60_di_remove() {
        let mut di = super::Xk60DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_60_di_covered_length() {
        let mut di = super::Xk60DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_60_di_gaps() {
        let mut di = super::Xk60DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_60_di_merge_adjacent() {
        let mut di = super::Xk60DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_60_di_empty() {
        let di = super::Xk60DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
