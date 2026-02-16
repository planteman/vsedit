//! Enterprise policy enforcement.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during policy operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// The requested policy was not found.
    PolicyNotFound(String),
    /// The policy value type does not match the expected type.
    TypeMismatch { policy: String, expected: &'static str },
    /// The policy is read-only and cannot be modified.
    ReadOnlyPolicy(String),
    /// The policy name is invalid (empty or contains invalid characters).
    InvalidPolicyName(String),
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyError::PolicyNotFound(name) => write!(f, "policy not found: {name}"),
            PolicyError::TypeMismatch { policy, expected } => {
                write!(f, "type mismatch for policy '{policy}': expected {expected}")
            }
            PolicyError::ReadOnlyPolicy(name) => write!(f, "policy is read-only: {name}"),
            PolicyError::InvalidPolicyName(name) => {
                write!(f, "invalid policy name: '{name}'")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A typed policy value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyValue {
    Bool(bool),
    String(String),
    Number(i64),
    StringList(Vec<String>),
}

impl fmt::Display for PolicyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyValue::Bool(v) => write!(f, "{v}"),
            PolicyValue::String(v) => write!(f, "{v}"),
            PolicyValue::Number(v) => write!(f, "{v}"),
            PolicyValue::StringList(v) => write!(f, "[{}]", v.join(", ")),
        }
    }
}

/// A single named policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    pub name: String,
    pub value: PolicyValue,
    pub description: Option<String>,
}

impl fmt::Display for Policy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Policy({}={})", self.name, self.value)
    }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Manages enterprise policies keyed by name.
#[derive(Debug)]
pub struct PolicyService {
    policies: HashMap<String, Policy>,
    read_only: HashSet<String>,
}

impl PolicyService {
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
            read_only: HashSet::new(),
        }
    }

    /// Registers or updates a policy.
    pub fn set_policy(
        &mut self,
        name: impl Into<String>,
        value: PolicyValue,
        description: Option<String>,
    ) {
        let name = name.into();
        self.policies.insert(
            name.clone(),
            Policy {
                name,
                value,
                description,
            },
        );
    }

    /// Retrieves a policy by name.
    pub fn get_policy(&self, name: &str) -> Option<&Policy> {
        self.policies.get(name)
    }

    /// Convenience accessor for boolean policies.
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.policies.get(name)?.value {
            PolicyValue::Bool(v) => Some(v),
            _ => None,
        }
    }

    /// Convenience accessor for string policies.
    pub fn get_string(&self, name: &str) -> Option<&str> {
        match &self.policies.get(name)?.value {
            PolicyValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Returns `true` if the named feature is allowed.
    ///
    /// A feature is allowed when there is no policy restricting it, or when
    /// the corresponding boolean policy is `true`.
    pub fn is_allowed(&self, feature: &str) -> bool {
        match self.get_bool(feature) {
            Some(v) => v,
            None => true,
        }
    }

    /// Returns the number of registered policies.
    pub fn policy_count(&self) -> usize {
        self.policies.len()
    }

    /// Removes a policy by name.
    pub fn remove_policy(&mut self, name: &str) -> Result<Policy, PolicyError> {
        if self.read_only.contains(name) {
            return Err(PolicyError::ReadOnlyPolicy(name.to_string()));
        }
        self.policies
            .remove(name)
            .ok_or_else(|| PolicyError::PolicyNotFound(name.to_string()))
    }

    /// Convenience accessor for number policies.
    pub fn get_number(&self, name: &str) -> Option<i64> {
        match self.policies.get(name)?.value {
            PolicyValue::Number(v) => Some(v),
            _ => None,
        }
    }

    /// Convenience accessor for string list policies.
    pub fn get_string_list(&self, name: &str) -> Option<&[String]> {
        match &self.policies.get(name)?.value {
            PolicyValue::StringList(v) => Some(v.as_slice()),
            _ => None,
        }
    }

    /// Returns a sorted list of all policy names.
    pub fn list_policies(&self) -> Vec<String> {
        let mut names: Vec<String> = self.policies.keys().cloned().collect();
        names.sort();
        names
    }

    /// Merges another `PolicyService`'s policies into this one.
    ///
    /// Policies from `other` take precedence on name conflicts.
    pub fn merge_policies(&mut self, other: &PolicyService) {
        for (name, policy) in &other.policies {
            self.policies.insert(name.clone(), policy.clone());
        }
        for name in &other.read_only {
            self.read_only.insert(name.clone());
        }
    }

    /// Returns `true` if the named feature is restricted (inverse of `is_allowed`).
    pub fn is_restricted(&self, feature: &str) -> bool {
        !self.is_allowed(feature)
    }

    /// Sets a policy, but fails if the policy already exists and is read-only.
    pub fn try_set(
        &mut self,
        name: impl Into<String>,
        value: PolicyValue,
        description: Option<String>,
    ) -> Result<(), PolicyError> {
        let name = name.into();
        if name.is_empty() {
            return Err(PolicyError::InvalidPolicyName(name));
        }
        if self.read_only.contains(&name) {
            return Err(PolicyError::ReadOnlyPolicy(name));
        }
        self.policies.insert(
            name.clone(),
            Policy {
                name,
                value,
                description,
            },
        );
        Ok(())
    }

    /// Marks an existing policy as read-only.
    pub fn mark_read_only(&mut self, name: &str) -> Result<(), PolicyError> {
        if !self.policies.contains_key(name) {
            return Err(PolicyError::PolicyNotFound(name.to_string()));
        }
        self.read_only.insert(name.to_string());
        Ok(())
    }
}

impl Default for PolicyService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for policy operations.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl PolicyStats {
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
    pub fn merge(&mut self, other: &PolicyStats) {
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

impl Default for PolicyStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PolicyStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PolicyStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for policy.
#[derive(Debug, Clone)]
pub struct PolicyValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl PolicyValidator {
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

impl Default for PolicyValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PolicyScope — scope of a policy
// ---------------------------------------------------------------------------

/// Scope at which a policy is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PolicyScope {
    User,
    Workspace,
    Machine,
}

impl PolicyScope {
    /// Returns the precedence level. Machine > Workspace > User.
    pub fn precedence(&self) -> u8 {
        match self {
            PolicyScope::User => 1,
            PolicyScope::Workspace => 2,
            PolicyScope::Machine => 3,
        }
    }
}

impl fmt::Display for PolicyScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyScope::User => write!(f, "User"),
            PolicyScope::Workspace => write!(f, "Workspace"),
            PolicyScope::Machine => write!(f, "Machine"),
        }
    }
}

// ---------------------------------------------------------------------------
// ScopedPolicy
// ---------------------------------------------------------------------------

/// A policy associated with a specific scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedPolicy {
    pub policy: Policy,
    pub scope: PolicyScope,
}

/// Merge scoped policies: higher-precedence scope wins on name conflict.
pub fn merge_scoped_policies(policies: &[ScopedPolicy]) -> Vec<Policy> {
    let mut best: HashMap<String, (u8, Policy)> = HashMap::new();
    for sp in policies {
        let prec = sp.scope.precedence();
        let entry = best.entry(sp.policy.name.clone());
        entry
            .and_modify(|(existing_prec, existing_policy)| {
                if prec > *existing_prec {
                    *existing_prec = prec;
                    *existing_policy = sp.policy.clone();
                }
            })
            .or_insert((prec, sp.policy.clone()));
    }
    let mut result: Vec<Policy> = best.into_values().map(|(_, p)| p).collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

// ---------------------------------------------------------------------------
// PolicyEngine — evaluate allow/deny rules with prefix matching
// ---------------------------------------------------------------------------

/// A rule in the policy engine.
#[derive(Debug, Clone)]
struct PolicyRule {
    pattern: String,
    allowed: bool,
}

/// Engine for evaluating allow/deny rules via prefix matching.
#[derive(Debug)]
pub struct PolicyEngine {
    rules: Vec<PolicyRule>,
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, pattern: &str, allowed: bool) {
        self.rules.push(PolicyRule {
            pattern: pattern.to_string(),
            allowed,
        });
    }

    /// Evaluate a feature against the rules. Most-specific (longest pattern) wins.
    pub fn evaluate(&self, feature: &str) -> Option<bool> {
        self.rules
            .iter()
            .filter(|r| feature.starts_with(&r.pattern))
            .max_by_key(|r| r.pattern.len())
            .map(|r| r.allowed)
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn clear(&mut self) {
        self.rules.clear();
    }

    pub fn remove_rule(&mut self, pattern: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.pattern != pattern);
        self.rules.len() < before
    }

    /// Check multiple features, returning results for each.
    pub fn matches_any<'a>(&self, features: &[&'a str]) -> Vec<(&'a str, bool)> {
        features
            .iter()
            .filter_map(|&f| self.evaluate(f).map(|v| (f, v)))
            .collect()
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_policy() {
        let mut svc = PolicyService::new();
        svc.set_policy("telemetry", PolicyValue::Bool(false), Some("Disable telemetry".into()));
        let p = svc.get_policy("telemetry").unwrap();
        assert_eq!(p.value, PolicyValue::Bool(false));
        assert_eq!(p.description.as_deref(), Some("Disable telemetry"));
        assert_eq!(svc.policy_count(), 1);
    }

    #[test]
    fn get_bool_and_string() {
        let mut svc = PolicyService::new();
        svc.set_policy("flag", PolicyValue::Bool(true), None);
        svc.set_policy("name", PolicyValue::String("acme".into()), None);
        assert_eq!(svc.get_bool("flag"), Some(true));
        assert_eq!(svc.get_string("name"), Some("acme"));
        // type mismatch returns None
        assert!(svc.get_bool("name").is_none());
        assert!(svc.get_string("flag").is_none());
    }

    #[test]
    fn is_allowed_logic() {
        let mut svc = PolicyService::new();
        // no policy → allowed
        assert!(svc.is_allowed("extensions"));
        // explicitly allowed
        svc.set_policy("extensions", PolicyValue::Bool(true), None);
        assert!(svc.is_allowed("extensions"));
        // explicitly denied
        svc.set_policy("extensions", PolicyValue::Bool(false), None);
        assert!(!svc.is_allowed("extensions"));
    }

    #[test]
    fn policy_count() {
        let mut svc = PolicyService::new();
        assert_eq!(svc.policy_count(), 0);
        svc.set_policy("a", PolicyValue::Number(42), None);
        svc.set_policy("b", PolicyValue::StringList(vec!["x".into()]), None);
        assert_eq!(svc.policy_count(), 2);
    }

    #[test]
    fn remove_policy_success() {
        let mut svc = PolicyService::new();
        svc.set_policy("tmp", PolicyValue::Bool(true), None);
        assert_eq!(svc.policy_count(), 1);
        let removed = svc.remove_policy("tmp").unwrap();
        assert_eq!(removed.name, "tmp");
        assert_eq!(svc.policy_count(), 0);
    }

    #[test]
    fn remove_policy_not_found() {
        let mut svc = PolicyService::new();
        let err = svc.remove_policy("missing").unwrap_err();
        assert_eq!(err, PolicyError::PolicyNotFound("missing".into()));
    }

    #[test]
    fn remove_read_only_policy_fails() {
        let mut svc = PolicyService::new();
        svc.set_policy("locked", PolicyValue::Bool(true), None);
        svc.mark_read_only("locked").unwrap();
        let err = svc.remove_policy("locked").unwrap_err();
        assert_eq!(err, PolicyError::ReadOnlyPolicy("locked".into()));
    }

    #[test]
    fn get_number_accessor() {
        let mut svc = PolicyService::new();
        svc.set_policy("max_tabs", PolicyValue::Number(10), None);
        assert_eq!(svc.get_number("max_tabs"), Some(10));
        // type mismatch
        svc.set_policy("flag", PolicyValue::Bool(true), None);
        assert!(svc.get_number("flag").is_none());
        // missing
        assert!(svc.get_number("nope").is_none());
    }

    #[test]
    fn get_string_list_accessor() {
        let mut svc = PolicyService::new();
        svc.set_policy(
            "allowed_hosts",
            PolicyValue::StringList(vec!["a.com".into(), "b.com".into()]),
            None,
        );
        assert_eq!(
            svc.get_string_list("allowed_hosts"),
            Some(vec!["a.com".to_string(), "b.com".to_string()].as_slice()),
        );
        // type mismatch
        svc.set_policy("flag", PolicyValue::Bool(false), None);
        assert!(svc.get_string_list("flag").is_none());
    }

    #[test]
    fn list_policies_sorted() {
        let mut svc = PolicyService::new();
        svc.set_policy("zebra", PolicyValue::Bool(true), None);
        svc.set_policy("alpha", PolicyValue::Number(1), None);
        svc.set_policy("middle", PolicyValue::String("m".into()), None);
        assert_eq!(svc.list_policies(), vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn merge_policies_other_wins() {
        let mut base = PolicyService::new();
        base.set_policy("a", PolicyValue::Number(1), None);
        base.set_policy("b", PolicyValue::Bool(true), None);

        let mut overlay = PolicyService::new();
        overlay.set_policy("a", PolicyValue::Number(99), Some("overridden".into()));
        overlay.set_policy("c", PolicyValue::String("new".into()), None);

        base.merge_policies(&overlay);
        assert_eq!(base.get_number("a"), Some(99));
        assert_eq!(base.get_bool("b"), Some(true));
        assert_eq!(base.get_string("c"), Some("new"));
        assert_eq!(base.policy_count(), 3);
    }

    #[test]
    fn merge_propagates_read_only() {
        let mut base = PolicyService::new();
        base.set_policy("x", PolicyValue::Bool(true), None);

        let mut overlay = PolicyService::new();
        overlay.set_policy("x", PolicyValue::Bool(false), None);
        overlay.mark_read_only("x").unwrap();

        base.merge_policies(&overlay);
        let err = base.try_set("x", PolicyValue::Bool(true), None).unwrap_err();
        assert_eq!(err, PolicyError::ReadOnlyPolicy("x".into()));
    }

    #[test]
    fn display_policy_value() {
        assert_eq!(format!("{}", PolicyValue::Bool(true)), "true");
        assert_eq!(format!("{}", PolicyValue::String("hello".into())), "hello");
        assert_eq!(format!("{}", PolicyValue::Number(-5)), "-5");
        assert_eq!(
            format!("{}", PolicyValue::StringList(vec!["a".into(), "b".into()])),
            "[a, b]"
        );
    }

    #[test]
    fn display_policy() {
        let p = Policy {
            name: "telemetry".into(),
            value: PolicyValue::Bool(false),
            description: None,
        };
        assert_eq!(format!("{p}"), "Policy(telemetry=false)");
    }

    #[test]
    fn is_restricted_inverse() {
        let mut svc = PolicyService::new();
        // no policy → not restricted
        assert!(!svc.is_restricted("feature"));
        svc.set_policy("feature", PolicyValue::Bool(false), None);
        assert!(svc.is_restricted("feature"));
        svc.set_policy("feature", PolicyValue::Bool(true), None);
        assert!(!svc.is_restricted("feature"));
    }

    #[test]
    fn try_set_read_only_rejected() {
        let mut svc = PolicyService::new();
        svc.set_policy("immutable", PolicyValue::Number(42), None);
        svc.mark_read_only("immutable").unwrap();
        let err = svc
            .try_set("immutable", PolicyValue::Number(99), None)
            .unwrap_err();
        assert_eq!(err, PolicyError::ReadOnlyPolicy("immutable".into()));
        // value unchanged
        assert_eq!(svc.get_number("immutable"), Some(42));
    }

    #[test]
    fn try_set_invalid_name() {
        let mut svc = PolicyService::new();
        let err = svc
            .try_set("", PolicyValue::Bool(true), None)
            .unwrap_err();
        assert_eq!(err, PolicyError::InvalidPolicyName(String::new()));
    }

    #[test]
    fn error_display_messages() {
        assert_eq!(
            PolicyError::PolicyNotFound("x".into()).to_string(),
            "policy not found: x"
        );
        assert_eq!(
            PolicyError::TypeMismatch {
                policy: "p".into(),
                expected: "bool"
            }
            .to_string(),
            "type mismatch for policy 'p': expected bool"
        );
        assert_eq!(
            PolicyError::ReadOnlyPolicy("r".into()).to_string(),
            "policy is read-only: r"
        );
        assert_eq!(
            PolicyError::InvalidPolicyName("".into()).to_string(),
            "invalid policy name: ''"
        );
    }

    #[test]
    fn display_policyerror_variants() {
        assert!(!PolicyError::PolicyNotFound("test".into()).to_string().is_empty());
    }

    // -- PolicyEngine tests --

    #[test]
    fn engine_empty_returns_none() {
        let engine = PolicyEngine::new();
        assert_eq!(engine.evaluate("anything"), None);
        assert_eq!(engine.rule_count(), 0);
    }

    #[test]
    fn engine_add_and_evaluate() {
        let mut engine = PolicyEngine::new();
        engine.add_rule("editor.", true);
        engine.add_rule("terminal.", false);
        assert_eq!(engine.evaluate("editor.fontSize"), Some(true));
        assert_eq!(engine.evaluate("terminal.shell"), Some(false));
        assert_eq!(engine.evaluate("unknown.feature"), None);
    }

    #[test]
    fn engine_most_specific_wins() {
        let mut engine = PolicyEngine::new();
        engine.add_rule("editor.", true);
        engine.add_rule("editor.font", false);
        assert_eq!(engine.evaluate("editor.fontSize"), Some(false));
        assert_eq!(engine.evaluate("editor.tabSize"), Some(true));
    }

    #[test]
    fn engine_remove_rule() {
        let mut engine = PolicyEngine::new();
        engine.add_rule("a", true);
        engine.add_rule("b", false);
        assert_eq!(engine.rule_count(), 2);
        assert!(engine.remove_rule("a"));
        assert_eq!(engine.rule_count(), 1);
        assert!(!engine.remove_rule("nonexistent"));
    }

    #[test]
    fn engine_clear() {
        let mut engine = PolicyEngine::new();
        engine.add_rule("x", true);
        engine.clear();
        assert_eq!(engine.rule_count(), 0);
    }

    #[test]
    fn engine_matches_any() {
        let mut engine = PolicyEngine::new();
        engine.add_rule("editor.", true);
        engine.add_rule("terminal.", false);
        let results = engine.matches_any(&["editor.font", "terminal.shell", "unknown"]);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], ("editor.font", true));
        assert_eq!(results[1], ("terminal.shell", false));
    }

    // -- PolicyScope tests --

    #[test]
    fn scope_precedence_ordering() {
        assert!(PolicyScope::Machine.precedence() > PolicyScope::Workspace.precedence());
        assert!(PolicyScope::Workspace.precedence() > PolicyScope::User.precedence());
        assert_eq!(PolicyScope::User.precedence(), 1);
        assert_eq!(PolicyScope::Machine.precedence(), 3);
    }

    #[test]
    fn scope_display() {
        assert_eq!(PolicyScope::User.to_string(), "User");
        assert_eq!(PolicyScope::Workspace.to_string(), "Workspace");
        assert_eq!(PolicyScope::Machine.to_string(), "Machine");
    }

    // -- merge_scoped_policies tests --

    #[test]
    fn merge_scoped_higher_wins() {
        let policies = vec![
            ScopedPolicy {
                policy: Policy {
                    name: "telemetry".into(),
                    value: PolicyValue::Bool(true),
                    description: None,
                },
                scope: PolicyScope::User,
            },
            ScopedPolicy {
                policy: Policy {
                    name: "telemetry".into(),
                    value: PolicyValue::Bool(false),
                    description: None,
                },
                scope: PolicyScope::Machine,
            },
            ScopedPolicy {
                policy: Policy {
                    name: "theme".into(),
                    value: PolicyValue::String("dark".into()),
                    description: None,
                },
                scope: PolicyScope::Workspace,
            },
        ];
        let merged = merge_scoped_policies(&policies);
        assert_eq!(merged.len(), 2);
        let telemetry = merged.iter().find(|p| p.name == "telemetry").unwrap();
        assert_eq!(telemetry.value, PolicyValue::Bool(false));
        let theme = merged.iter().find(|p| p.name == "theme").unwrap();
        assert_eq!(theme.value, PolicyValue::String("dark".into()));
    }

    #[test]
    fn merge_scoped_empty() {
        let merged = merge_scoped_policies(&[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn policy_stats_new_defaults() {
        let stats = PolicyStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn policy_stats_record_success() {
        let mut stats = PolicyStats::new();
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
    fn policy_stats_record_failure() {
        let mut stats = PolicyStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn policy_stats_reset() {
        let mut stats = PolicyStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn policy_stats_merge() {
        let mut a = PolicyStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = PolicyStats::new();
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
    fn policy_stats_display() {
        let mut stats = PolicyStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn policy_stats_default() {
        let stats = PolicyStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn policy_validator_accepts_valid_name() {
        let v = PolicyValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn policy_validator_rejects_empty() {
        let v = PolicyValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn policy_validator_rejects_too_long() {
        let v = PolicyValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn policy_validator_forbidden_prefix() {
        let v = PolicyValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn policy_validator_allowed_chars() {
        let v = PolicyValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn policy_validator_range() {
        let v = PolicyValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn policy_sanitize_removes_control() {
        let result = PolicyValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn policy_truncate_short_string() {
        assert_eq!(PolicyValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn policy_truncate_long_string() {
        let result = PolicyValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn policy_is_ascii_printable() {
        assert!(PolicyValidator::is_ascii_printable("Hello World 123"));
        assert!(!PolicyValidator::is_ascii_printable("Hello\x00World"));
    }
}
