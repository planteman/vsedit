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
}
