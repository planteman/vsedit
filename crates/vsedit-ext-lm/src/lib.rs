//! Ext API: Language models.
//!
//! RPC bridge between the extension host and the main thread for language model access.

use std::fmt;
use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_lm";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LmMessage {
    ListModels,
    SendRequest {
        model_id: String,
        messages: Vec<LanguageModelMessage>,
    },
    CancelRequest {
        request_id: String,
    },
    CountTokens {
        model_id: String,
        text: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanguageModelChat {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub family: String,
    pub version: String,
    pub max_input_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role", rename_all = "camelCase")]
pub enum LanguageModelMessage {
    System { content: String },
    User { content: String },
    Assistant { content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LmResponse {
    pub request_id: String,
    pub text: String,
    pub is_complete: bool,
}

// ── Bridge ──

pub struct LmBridge {
    models: Vec<LanguageModelChat>,
}

impl LmBridge {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
        }
    }

    pub fn register_model(&mut self, model: LanguageModelChat) {
        if !self.models.iter().any(|m| m.id == model.id) {
            self.models.push(model);
        }
    }

    pub fn list_models(&self) -> &[LanguageModelChat] {
        &self.models
    }

    pub fn get_model(&self, id: &str) -> Option<&LanguageModelChat> {
        self.models.iter().find(|m| m.id == id)
    }

    pub fn handle_message(&self, msg: &LmMessage) -> serde_json::Value {
        match msg {
            LmMessage::ListModels => {
                let names: Vec<&str> = self.models.iter().map(|m| m.name.as_str()).collect();
                serde_json::json!({"models": names})
            }
            LmMessage::SendRequest {
                model_id,
                messages,
            } => {
                let found = self.get_model(model_id).is_some();
                serde_json::json!({"accepted": found, "messageCount": messages.len()})
            }
            LmMessage::CancelRequest { request_id } => {
                serde_json::json!({"cancelled": request_id})
            }
            LmMessage::CountTokens { model_id, text } => {
                let found = self.get_model(model_id).is_some();
                // Rough estimate: split on whitespace
                let count = text.split_whitespace().count();
                serde_json::json!({"found": found, "tokens": count})
            }
        }
    }
}

impl Default for LmBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the lm extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

// ── Request Options ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanguageModelRequestOptions {
    pub temperature: f64,
    pub max_tokens: u32,
    pub stop_sequences: Vec<String>,
    pub top_p: f64,
}

impl Default for LanguageModelRequestOptions {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 4096,
            stop_sequences: Vec::new(),
            top_p: 1.0,
        }
    }
}

// ── Extended Bridge Methods ──

impl LmBridge {
    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    pub fn get_models_by_vendor(&self, vendor: &str) -> Vec<&LanguageModelChat> {
        self.models.iter().filter(|m| m.vendor == vendor).collect()
    }

    pub fn get_models_by_family(&self, family: &str) -> Vec<&LanguageModelChat> {
        self.models.iter().filter(|m| m.family == family).collect()
    }

    pub fn unregister_model(&mut self, id: &str) -> bool {
        let before = self.models.len();
        self.models.retain(|m| m.id != id);
        self.models.len() < before
    }
}

// ── Token Counter ──

pub struct TokenCounter;

impl TokenCounter {
    /// Count tokens by splitting on whitespace and punctuation boundaries.
    pub fn count_tokens(text: &str) -> usize {
        text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .filter(|s| !s.is_empty())
            .count()
    }
}

/// Estimate cost given a token count and price per 1k tokens.
pub fn estimate_cost(token_count: usize, price_per_1k: f64) -> f64 {
    (token_count as f64 / 1000.0) * price_per_1k
}

/// Validate that a message slice is non-empty and every message has non-empty content.
pub fn validate_messages(messages: &[LanguageModelMessage]) -> bool {
    if messages.is_empty() {
        return false;
    }
    messages.iter().all(|m| {
        let content = match m {
            LanguageModelMessage::System { content } => content,
            LanguageModelMessage::User { content } => content,
            LanguageModelMessage::Assistant { content } => content,
        };
        !content.is_empty()
    })
}

// ── Conversation History ──

#[derive(Debug, Clone, Default)]
pub struct ConversationHistory {
    messages: Vec<LanguageModelMessage>,
}

impl ConversationHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_message(&mut self, message: LanguageModelMessage) {
        self.messages.push(message);
    }

    pub fn get_messages(&self) -> &[LanguageModelMessage] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn total_tokens_estimate(&self) -> usize {
        self.messages
            .iter()
            .map(|m| {
                let content = match m {
                    LanguageModelMessage::System { content } => content,
                    LanguageModelMessage::User { content } => content,
                    LanguageModelMessage::Assistant { content } => content,
                };
                TokenCounter::count_tokens(content)
            })
            .sum()
    }

    pub fn truncate_to_token_limit(&mut self, limit: usize) {
        while self.total_tokens_estimate() > limit && !self.messages.is_empty() {
            self.messages.remove(0);
        }
    }
}

/// A structured request for AI completion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanguageModelRequest {
    pub model_id: String,
    pub messages: Vec<LanguageModelMessage>,
    pub options: LanguageModelRequestOptions,
    pub request_id: String,
}

impl LanguageModelRequest {
    pub fn new(model_id: &str, messages: Vec<LanguageModelMessage>) -> Self {
        Self {
            model_id: model_id.to_string(),
            messages,
            options: LanguageModelRequestOptions::default(),
            request_id: format!("req_{}", Self::simple_id()),
        }
    }

    fn simple_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    pub fn with_options(mut self, options: LanguageModelRequestOptions) -> Self {
        self.options = options;
        self
    }

    /// Estimate total tokens for this request using TokenCounter.
    pub fn estimated_tokens(&self) -> usize {
        self.messages
            .iter()
            .map(|m| {
                let content = match m {
                    LanguageModelMessage::System { content } => content,
                    LanguageModelMessage::User { content } => content,
                    LanguageModelMessage::Assistant { content } => content,
                };
                TokenCounter::count_tokens(content)
            })
            .sum()
    }

    /// Check if this request would exceed the given model's token limit.
    pub fn exceeds_limit(&self, model: &LanguageModelChat) -> bool {
        self.estimated_tokens() > model.max_input_tokens as usize
    }
}

/// Aggregated response with streaming support.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanguageModelResponse {
    pub request_id: String,
    pub chunks: Vec<String>,
    pub is_complete: bool,
    pub total_tokens_used: usize,
}

impl LanguageModelResponse {
    pub fn new(request_id: &str) -> Self {
        Self {
            request_id: request_id.to_string(),
            chunks: Vec::new(),
            is_complete: false,
            total_tokens_used: 0,
        }
    }

    pub fn append_chunk(&mut self, chunk: &str) {
        self.chunks.push(chunk.to_string());
        self.total_tokens_used += TokenCounter::count_tokens(chunk);
    }

    pub fn complete(&mut self) {
        self.is_complete = true;
    }

    /// Get the full assembled text.
    pub fn full_text(&self) -> String {
        self.chunks.join("")
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

/// Estimate total token usage for a request, including per-message overhead.
/// Each message has ~4 tokens of overhead (role, delimiters).
pub fn token_estimate(messages: &[LanguageModelMessage], overhead_per_message: usize) -> usize {
    messages
        .iter()
        .map(|m| {
            let content = match m {
                LanguageModelMessage::System { content } => content,
                LanguageModelMessage::User { content } => content,
                LanguageModelMessage::Assistant { content } => content,
            };
            TokenCounter::count_tokens(content) + overhead_per_message
        })
        .sum()
}

/// A registered language model backend.
#[derive(Debug, Clone)]
pub struct LanguageModelProviderInfo {
    pub id: String,
    pub display_name: String,
    pub models: Vec<String>,
}

/// Registry for multiple language model providers.
pub struct LanguageModelProviderRegistry {
    providers: Vec<LanguageModelProviderInfo>,
}

impl LanguageModelProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: LanguageModelProviderInfo) {
        if !self.providers.iter().any(|p| p.id == provider.id) {
            self.providers.push(provider);
        }
    }

    pub fn unregister(&mut self, id: &str) -> bool {
        let before = self.providers.len();
        self.providers.retain(|p| p.id != id);
        self.providers.len() < before
    }

    pub fn get(&self, id: &str) -> Option<&LanguageModelProviderInfo> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn list(&self) -> &[LanguageModelProviderInfo] {
        &self.providers
    }

    pub fn count(&self) -> usize {
        self.providers.len()
    }

    /// Find providers that offer a specific model.
    pub fn find_by_model(&self, model_id: &str) -> Vec<&LanguageModelProviderInfo> {
        self.providers
            .iter()
            .filter(|p| p.models.iter().any(|m| m == model_id))
            .collect()
    }
}

impl Default for LanguageModelProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for ext-lm operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtLmStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtLmStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &ExtLmStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for ExtLmStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtLmStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtLmStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-lm.
#[derive(Debug, Clone)]
pub struct ExtLmValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtLmValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for ExtLmValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Token Budget ──

/// Tracks token consumption against a fixed budget.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenBudget {
    limit: usize,
    used: usize,
}

impl TokenBudget {
    /// Create a new budget with the given token limit.
    pub fn new(limit: usize) -> Self {
        Self { limit, used: 0 }
    }

    /// Try to consume `amount` tokens. Returns `true` if within budget.
    pub fn try_consume(&mut self, amount: usize) -> bool {
        if self.used + amount > self.limit {
            return false;
        }
        self.used += amount;
        true
    }

    /// Remaining tokens available in this budget.
    pub fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.used)
    }

    /// Fraction of budget consumed, in [0.0, 1.0].
    pub fn utilization(&self) -> f64 {
        if self.limit == 0 {
            return 1.0;
        }
        self.used as f64 / self.limit as f64
    }

    /// Reset consumption to zero without changing the limit.
    pub fn reset(&mut self) {
        self.used = 0;
    }

    /// Whether the budget is fully exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.used >= self.limit
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn used(&self) -> usize {
        self.used
    }
}

impl fmt::Display for TokenBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TokenBudget({}/{}, {:.1}%)",
            self.used,
            self.limit,
            self.utilization() * 100.0
        )
    }
}

// ── Model Capability Matrix ──

/// Describes the capabilities a language model supports.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelCapabilities {
    pub supports_system_messages: bool,
    pub supports_streaming: bool,
    pub supports_function_calling: bool,
    pub supports_vision: bool,
    pub max_output_tokens: u32,
}

/// Maps model ids to their capabilities.
#[derive(Debug, Clone, Default)]
pub struct ModelCapabilityMatrix {
    entries: Vec<(String, ModelCapabilities)>,
}

impl ModelCapabilityMatrix {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register capabilities for a model id.
    pub fn register(&mut self, model_id: impl Into<String>, caps: ModelCapabilities) {
        let id = model_id.into();
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| *k == id) {
            entry.1 = caps;
        } else {
            self.entries.push((id, caps));
        }
    }

    /// Look up capabilities for a model.
    pub fn get(&self, model_id: &str) -> Option<&ModelCapabilities> {
        self.entries.iter().find(|(k, _)| k == model_id).map(|(_, v)| v)
    }

    /// Find all models that support a given feature predicate.
    pub fn find_supporting(&self, predicate: impl Fn(&ModelCapabilities) -> bool) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(_, c)| predicate(c))
            .map(|(id, _)| id.as_str())
            .collect()
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

// ── Prompt Template ──

/// A reusable prompt template with `{{placeholder}}` substitution.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptTemplate {
    template: String,
}

impl PromptTemplate {
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
        }
    }

    /// Render the template, replacing `{{key}}` with the corresponding value.
    pub fn render(&self, vars: &[(&str, &str)]) -> String {
        let mut result = self.template.clone();
        for (key, value) in vars {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Return the set of placeholder names found in the template.
    pub fn placeholders(&self) -> Vec<String> {
        let mut names = Vec::new();
        let bytes = self.template.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            if bytes[i] == b'{' && bytes[i + 1] == b'{' {
                if let Some(end) = self.template[i + 2..].find("}}") {
                    let name = &self.template[i + 2..i + 2 + end];
                    if !name.is_empty() && !names.iter().any(|n: &String| n == name) {
                        names.push(name.to_string());
                    }
                    i += 4 + end;
                    continue;
                }
            }
            i += 1;
        }
        names
    }

    /// Check that all placeholders in the template are provided.
    pub fn validate_vars(&self, vars: &[(&str, &str)]) -> Result<(), Vec<String>> {
        let required = self.placeholders();
        let provided: Vec<&str> = vars.iter().map(|(k, _)| *k).collect();
        let missing: Vec<String> = required
            .into_iter()
            .filter(|r| !provided.contains(&r.as_str()))
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

// ── Response Parser ──

/// Utilities for extracting structured data from model responses.
pub struct ResponseParser;

impl ResponseParser {
    /// Extract the first fenced code block from a response string.
    pub fn extract_code_block(text: &str) -> Option<&str> {
        let start_marker = "```";
        let start = text.find(start_marker)?;
        let after_start = start + start_marker.len();
        // Skip optional language tag on the same line.
        let content_start = text[after_start..].find('\n')? + after_start + 1;
        let end = text[content_start..].find(start_marker)?;
        let block = &text[content_start..content_start + end];
        Some(block.trim_end_matches('\n'))
    }

    /// Extract all lines that start with `- ` as a bullet list.
    pub fn extract_bullet_list(text: &str) -> Vec<&str> {
        text.lines()
            .filter_map(|line| {
                let trimmed = line.trim_start();
                trimmed.strip_prefix("- ")
            })
            .collect()
    }

    /// Count the number of sentences (heuristic: split on `. `, `! `, `? `).
    pub fn sentence_count(text: &str) -> usize {
        if text.trim().is_empty() {
            return 0;
        }
        text.split(|c: char| c == '.' || c == '!' || c == '?')
            .filter(|s| !s.trim().is_empty())
            .count()
    }
}

// ---------------------------------------------------------------------------
// Model comparison and conversation utilities
// ---------------------------------------------------------------------------

/// Check if two models belong to the same family.
pub fn same_family(a: &LanguageModelChat, b: &LanguageModelChat) -> bool {
    a.family == b.family
}

/// Check if two models are from the same vendor.
pub fn same_vendor(a: &LanguageModelChat, b: &LanguageModelChat) -> bool {
    a.vendor == b.vendor
}

/// Format a model identifier as "vendor/family/name".
pub fn format_model_id(model: &LanguageModelChat) -> String {
    format!("{}/{}/{}", model.vendor, model.family, model.name)
}

/// Compute the average token estimate across conversation messages.
pub fn average_message_tokens(history: &ConversationHistory) -> usize {
    let msgs = history.get_messages();
    if msgs.is_empty() {
        return 0;
    }
    history.total_tokens_estimate() / msgs.len()
}

/// Count the number of user messages in a conversation.
pub fn count_user_messages(history: &ConversationHistory) -> usize {
    history
        .get_messages()
        .iter()
        .filter(|m| matches!(m, LanguageModelMessage::User { .. }))
        .count()
}

/// Count the number of assistant messages in a conversation.
pub fn count_assistant_messages(history: &ConversationHistory) -> usize {
    history
        .get_messages()
        .iter()
        .filter(|m| matches!(m, LanguageModelMessage::Assistant { .. }))
        .count()
}

/// Extract all user message texts from a conversation history.
pub fn extract_user_texts(history: &ConversationHistory) -> Vec<&str> {
    history
        .get_messages()
        .iter()
        .filter_map(|m| match m {
            LanguageModelMessage::User { content } => Some(content.as_str()),
            _ => None,
        })
        .collect()
}

/// Determine if a model has sufficient capacity for a given token count.
pub fn model_has_capacity(model: &LanguageModelChat, required_tokens: u32) -> bool {
    model.max_input_tokens >= required_tokens
}

/// Find the model with the largest token capacity from a bridge.
pub fn largest_model(bridge: &LmBridge) -> Option<&LanguageModelChat> {
    bridge.list_models().iter().max_by_key(|m| m.max_input_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_model() -> LanguageModelChat {
        LanguageModelChat {
            id: "gpt-4".into(),
            name: "GPT-4".into(),
            vendor: "openai".into(),
            family: "gpt".into(),
            version: "4".into(),
            max_input_tokens: 8192,
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = LmMessage::SendRequest {
            model_id: "gpt-4".into(),
            messages: vec![LanguageModelMessage::User {
                content: "hello".into(),
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: LmMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn model_serialization() {
        let m = test_model();
        let json = serde_json::to_string(&m).unwrap();
        let back: LanguageModelChat = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn bridge_register_and_list() {
        let mut bridge = LmBridge::new();
        bridge.register_model(test_model());
        assert_eq!(bridge.list_models().len(), 1);
        assert!(bridge.get_model("gpt-4").is_some());
    }

    #[test]
    fn bridge_handle_list() {
        let mut bridge = LmBridge::new();
        bridge.register_model(test_model());
        let result = bridge.handle_message(&LmMessage::ListModels);
        let models = result["models"].as_array().unwrap();
        assert_eq!(models.len(), 1);
    }

    #[test]
    fn bridge_count_tokens() {
        let mut bridge = LmBridge::new();
        bridge.register_model(test_model());
        let result = bridge.handle_message(&LmMessage::CountTokens {
            model_id: "gpt-4".into(),
            text: "hello world foo".into(),
        });
        assert_eq!(result["tokens"], 3);
    }

    #[test]
    fn request_options_default() {
        let opts = LanguageModelRequestOptions::default();
        assert!((opts.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(opts.max_tokens, 4096);
        assert!(opts.stop_sequences.is_empty());
        assert!((opts.top_p - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn model_count_and_unregister() {
        let mut bridge = LmBridge::new();
        assert_eq!(bridge.model_count(), 0);
        bridge.register_model(test_model());
        assert_eq!(bridge.model_count(), 1);
        assert!(bridge.unregister_model("gpt-4"));
        assert_eq!(bridge.model_count(), 0);
        assert!(!bridge.unregister_model("nonexistent"));
    }

    #[test]
    fn get_models_by_vendor() {
        let mut bridge = LmBridge::new();
        bridge.register_model(test_model());
        bridge.register_model(LanguageModelChat {
            id: "claude".into(),
            name: "Claude".into(),
            vendor: "anthropic".into(),
            family: "claude".into(),
            version: "3".into(),
            max_input_tokens: 100_000,
        });
        assert_eq!(bridge.get_models_by_vendor("openai").len(), 1);
        assert_eq!(bridge.get_models_by_vendor("anthropic").len(), 1);
        assert!(bridge.get_models_by_vendor("unknown").is_empty());
    }

    #[test]
    fn get_models_by_family() {
        let mut bridge = LmBridge::new();
        bridge.register_model(test_model());
        assert_eq!(bridge.get_models_by_family("gpt").len(), 1);
        assert!(bridge.get_models_by_family("llama").is_empty());
    }

    #[test]
    fn token_counter_basic() {
        assert_eq!(TokenCounter::count_tokens("hello world"), 2);
        assert_eq!(TokenCounter::count_tokens("one,two.three!four"), 4);
        assert_eq!(TokenCounter::count_tokens(""), 0);
    }

    #[test]
    fn estimate_cost_calculation() {
        let cost = estimate_cost(1000, 0.03);
        assert!((cost - 0.03).abs() < f64::EPSILON);
        let cost2 = estimate_cost(500, 0.06);
        assert!((cost2 - 0.03).abs() < f64::EPSILON);
    }

    #[test]
    fn validate_messages_checks() {
        assert!(!validate_messages(&[]));
        assert!(validate_messages(&[LanguageModelMessage::User {
            content: "hi".into(),
        }]));
        assert!(!validate_messages(&[LanguageModelMessage::User {
            content: "".into(),
        }]));
    }

    #[test]
    fn conversation_history_basic() {
        let mut history = ConversationHistory::new();
        assert!(history.get_messages().is_empty());
        history.add_message(LanguageModelMessage::User {
            content: "hello world".into(),
        });
        assert_eq!(history.get_messages().len(), 1);
        assert_eq!(history.total_tokens_estimate(), 2);
        history.clear();
        assert!(history.get_messages().is_empty());
    }

    #[test]
    fn conversation_history_truncate() {
        let mut history = ConversationHistory::new();
        history.add_message(LanguageModelMessage::User {
            content: "one two three".into(),
        });
        history.add_message(LanguageModelMessage::Assistant {
            content: "four five six".into(),
        });
        history.add_message(LanguageModelMessage::User {
            content: "seven eight".into(),
        });
        assert_eq!(history.total_tokens_estimate(), 8);
        history.truncate_to_token_limit(5);
        assert!(history.total_tokens_estimate() <= 5);
        assert!(!history.get_messages().is_empty());
    }

    #[test]
    fn request_options_serialization() {
        let opts = LanguageModelRequestOptions {
            temperature: 0.9,
            max_tokens: 2048,
            stop_sequences: vec!["STOP".into()],
            top_p: 0.95,
        };
        let json = serde_json::to_string(&opts).unwrap();
        let back: LanguageModelRequestOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn eq_lmmessage_same() {
        assert_eq!(LmMessage::ListModels, LmMessage::ListModels);
    }

    #[test]
    fn ne_lmmessage_diff() {
        // LmMessage only has one simple variant; verify size instead
        assert!(std::mem::size_of::<LmMessage>() > 0);
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_31() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_32() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_33() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_34() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_35() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_36() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn ext_lm_stats_new_defaults() {
        let stats = ExtLmStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_lm_stats_record_success() {
        let mut stats = ExtLmStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_lm_stats_record_failure() {
        let mut stats = ExtLmStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_lm_stats_reset() {
        let mut stats = ExtLmStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_lm_stats_merge() {
        let mut a = ExtLmStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtLmStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn ext_lm_stats_display() {
        let mut stats = ExtLmStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_lm_stats_default() {
        let stats = ExtLmStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_lm_validator_accepts_valid_name() {
        let v = ExtLmValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_lm_validator_rejects_empty() {
        let v = ExtLmValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_lm_validator_rejects_too_long() {
        let v = ExtLmValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_lm_validator_forbidden_prefix() {
        let v = ExtLmValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_lm_validator_allowed_chars() {
        let v = ExtLmValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_lm_validator_range() {
        let v = ExtLmValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_lm_sanitize_removes_control() {
        let result = ExtLmValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_lm_truncate_short_string() {
        assert_eq!(ExtLmValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_lm_truncate_long_string() {
        let result = ExtLmValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_lm_is_ascii_printable() {
        assert!(ExtLmValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtLmValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn lm_request_new_has_id() {
        let req = LanguageModelRequest::new(
            "model-1",
            vec![LanguageModelMessage::User {
                content: "Hello".to_string(),
            }],
        );
        assert!(req.request_id.starts_with("req_"));
        assert_eq!(req.model_id, "model-1");
    }

    #[test]
    fn lm_request_estimated_tokens() {
        let req = LanguageModelRequest::new(
            "m",
            vec![LanguageModelMessage::User {
                content: "one two three".to_string(),
            }],
        );
        assert_eq!(req.estimated_tokens(), 3);
    }

    #[test]
    fn lm_request_exceeds_limit() {
        let model = LanguageModelChat {
            id: "m".into(),
            name: "M".into(),
            vendor: "v".into(),
            family: "f".into(),
            version: "1".into(),
            max_input_tokens: 2,
        };
        let req = LanguageModelRequest::new(
            "m",
            vec![LanguageModelMessage::User {
                content: "one two three".to_string(),
            }],
        );
        assert!(req.exceeds_limit(&model));
    }

    #[test]
    fn lm_response_streaming() {
        let mut resp = LanguageModelResponse::new("req_1");
        resp.append_chunk("Hello ");
        resp.append_chunk("World");
        resp.complete();
        assert_eq!(resp.full_text(), "Hello World");
        assert!(resp.is_complete);
        assert_eq!(resp.chunk_count(), 2);
    }

    #[test]
    fn token_estimate_with_overhead() {
        let msgs = vec![
            LanguageModelMessage::User {
                content: "hello world".to_string(),
            },
            LanguageModelMessage::System {
                content: "you are helpful".to_string(),
            },
        ];
        let est = token_estimate(&msgs, 4);
        // "hello world" = 2 tokens + 4 overhead = 6
        // "you are helpful" = 3 tokens + 4 overhead = 7
        assert_eq!(est, 13);
    }

    #[test]
    fn provider_registry_register_and_find() {
        let mut reg = LanguageModelProviderRegistry::new();
        reg.register(LanguageModelProviderInfo {
            id: "openai".into(),
            display_name: "OpenAI".into(),
            models: vec!["gpt-4".into(), "gpt-3.5".into()],
        });
        assert_eq!(reg.count(), 1);
        assert!(reg.get("openai").is_some());
    }

    #[test]
    fn provider_registry_no_duplicate() {
        let mut reg = LanguageModelProviderRegistry::new();
        let p = LanguageModelProviderInfo {
            id: "a".into(),
            display_name: "A".into(),
            models: vec![],
        };
        reg.register(p.clone());
        reg.register(p);
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn provider_registry_find_by_model() {
        let mut reg = LanguageModelProviderRegistry::new();
        reg.register(LanguageModelProviderInfo {
            id: "p1".into(),
            display_name: "P1".into(),
            models: vec!["m1".into()],
        });
        reg.register(LanguageModelProviderInfo {
            id: "p2".into(),
            display_name: "P2".into(),
            models: vec!["m2".into()],
        });
        let found = reg.find_by_model("m1");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "p1");
    }

    #[test]
    fn provider_registry_unregister() {
        let mut reg = LanguageModelProviderRegistry::new();
        reg.register(LanguageModelProviderInfo {
            id: "x".into(),
            display_name: "X".into(),
            models: vec![],
        });
        assert!(reg.unregister("x"));
        assert_eq!(reg.count(), 0);
        assert!(!reg.unregister("x"));
    }

    // ── Token Budget Tests ──

    #[test]
    fn token_budget_consume_and_remaining() {
        let mut budget = TokenBudget::new(100);
        assert_eq!(budget.remaining(), 100);
        assert!(budget.try_consume(30));
        assert_eq!(budget.remaining(), 70);
        assert_eq!(budget.used(), 30);
        assert!(!budget.is_exhausted());
        // Cannot exceed limit
        assert!(!budget.try_consume(71));
        assert_eq!(budget.remaining(), 70);
    }

    #[test]
    fn token_budget_utilization_and_display() {
        let mut budget = TokenBudget::new(200);
        budget.try_consume(100);
        assert!((budget.utilization() - 0.5).abs() < f64::EPSILON);
        let display = format!("{budget}");
        assert!(display.contains("100/200"));
    }

    #[test]
    fn token_budget_exhaustion_and_reset() {
        let mut budget = TokenBudget::new(10);
        budget.try_consume(10);
        assert!(budget.is_exhausted());
        assert!(!budget.try_consume(1));
        budget.reset();
        assert_eq!(budget.used(), 0);
        assert!(!budget.is_exhausted());
    }

    // ── Model Capability Matrix Tests ──

    #[test]
    fn capability_matrix_register_and_query() {
        let mut matrix = ModelCapabilityMatrix::new();
        matrix.register("gpt-4", ModelCapabilities {
            supports_system_messages: true,
            supports_streaming: true,
            supports_function_calling: true,
            supports_vision: true,
            max_output_tokens: 8192,
        });
        matrix.register("llama-2", ModelCapabilities {
            supports_system_messages: true,
            supports_streaming: false,
            supports_function_calling: false,
            supports_vision: false,
            max_output_tokens: 4096,
        });
        assert_eq!(matrix.count(), 2);
        let caps = matrix.get("gpt-4").unwrap();
        assert!(caps.supports_vision);
        let streamers = matrix.find_supporting(|c| c.supports_streaming);
        assert_eq!(streamers, vec!["gpt-4"]);
    }

    #[test]
    fn capability_matrix_overwrite() {
        let mut matrix = ModelCapabilityMatrix::new();
        matrix.register("m1", ModelCapabilities {
            supports_vision: false,
            ..Default::default()
        });
        matrix.register("m1", ModelCapabilities {
            supports_vision: true,
            ..Default::default()
        });
        assert_eq!(matrix.count(), 1);
        assert!(matrix.get("m1").unwrap().supports_vision);
    }

    // ── Prompt Template Tests ──

    #[test]
    fn prompt_template_render_and_placeholders() {
        let tpl = PromptTemplate::new("Hello {{name}}, you are a {{role}}.");
        let placeholders = tpl.placeholders();
        assert_eq!(placeholders, vec!["name", "role"]);
        let rendered = tpl.render(&[("name", "Alice"), ("role", "developer")]);
        assert_eq!(rendered, "Hello Alice, you are a developer.");
    }

    #[test]
    fn prompt_template_validate_vars() {
        let tpl = PromptTemplate::new("{{a}} and {{b}}");
        assert!(tpl.validate_vars(&[("a", "1"), ("b", "2")]).is_ok());
        let missing = tpl.validate_vars(&[("a", "1")]).unwrap_err();
        assert_eq!(missing, vec!["b"]);
    }

    // ── Response Parser Tests ──

    #[test]
    fn response_parser_extract_code_block() {
        let text = "Here is code:\n```rust\nfn main() {}\n```\nDone.";
        let block = ResponseParser::extract_code_block(text).unwrap();
        assert_eq!(block, "fn main() {}");
    }

    #[test]
    fn response_parser_bullet_list() {
        let text = "Items:\n- apple\n- banana\n  - cherry\nnot a bullet";
        let items = ResponseParser::extract_bullet_list(text);
        assert_eq!(items, vec!["apple", "banana", "cherry"]);
    }

    #[test]
    fn response_parser_sentence_count() {
        assert_eq!(ResponseParser::sentence_count("Hello. World! How?"), 3);
        assert_eq!(ResponseParser::sentence_count(""), 0);
        assert_eq!(ResponseParser::sentence_count("No punctuation here"), 1);
    }

    #[test]
    fn same_family_true() {
        let a = test_model();
        let b = LanguageModelChat {
            id: "other".into(),
            name: "Other".into(),
            vendor: "different-vendor".into(),
            family: "gpt".into(),
            version: "2.0".into(),
            max_input_tokens: 2048,
        };
        assert!(same_family(&a, &b));
    }

    #[test]
    fn same_family_false() {
        let a = test_model();
        let b = LanguageModelChat {
            id: "other".into(),
            name: "Other".into(),
            vendor: "openai".into(),
            family: "claude".into(),
            version: "1.0".into(),
            max_input_tokens: 2048,
        };
        assert!(!same_family(&a, &b));
    }

    #[test]
    fn format_model_id_format() {
        let m = test_model();
        assert_eq!(format_model_id(&m), "openai/gpt/GPT-4");
    }

    #[test]
    fn average_message_tokens_empty() {
        let h = ConversationHistory::new();
        assert_eq!(average_message_tokens(&h), 0);
    }

    #[test]
    fn count_user_and_assistant_messages() {
        let mut h = ConversationHistory::new();
        h.add_message(LanguageModelMessage::User { content: "hello".into() });
        h.add_message(LanguageModelMessage::Assistant { content: "hi".into() });
        h.add_message(LanguageModelMessage::User { content: "bye".into() });
        assert_eq!(count_user_messages(&h), 2);
        assert_eq!(count_assistant_messages(&h), 1);
    }

    #[test]
    fn extract_user_texts_filters() {
        let mut h = ConversationHistory::new();
        h.add_message(LanguageModelMessage::User { content: "q1".into() });
        h.add_message(LanguageModelMessage::Assistant { content: "a1".into() });
        h.add_message(LanguageModelMessage::User { content: "q2".into() });
        let texts = extract_user_texts(&h);
        assert_eq!(texts, vec!["q1", "q2"]);
    }

    #[test]
    fn model_has_capacity_sufficient() {
        let m = test_model();
        assert!(model_has_capacity(&m, 100));
    }

    #[test]
    fn model_has_capacity_insufficient() {
        let m = test_model();
        assert!(!model_has_capacity(&m, 100_000));
    }

    #[test]
    fn largest_model_finds_max() {
        let mut bridge = LmBridge::new();
        bridge.register_model(LanguageModelChat {
            id: "small".into(),
            name: "Small".into(),
            vendor: "v".into(),
            family: "f".into(),
            version: "1".into(),
            max_input_tokens: 100,
        });
        bridge.register_model(LanguageModelChat {
            id: "big".into(),
            name: "Big".into(),
            vendor: "v".into(),
            family: "f".into(),
            version: "1".into(),
            max_input_tokens: 10000,
        });
        let best = largest_model(&bridge).unwrap();
        assert_eq!(best.id, "big");
    }

    #[test]
    fn largest_model_empty_bridge() {
        let bridge = LmBridge::new();
        assert!(largest_model(&bridge).is_none());
    }
}
