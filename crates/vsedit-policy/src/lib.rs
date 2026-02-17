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

// ---------------------------------------------------------------------------
// policy_report — human-readable policy summary
// ---------------------------------------------------------------------------

/// A single line in a policy report.
#[derive(Debug, Clone)]
pub struct PolicyReportLine {
    pub name: String,
    pub value: String,
    pub scope: String,
    pub read_only: bool,
}

impl fmt::Display for PolicyReportLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ro = if self.read_only { " [read-only]" } else { "" };
        write!(f, "{}: {} (scope: {}){}", self.name, self.value, self.scope, ro)
    }
}

impl PolicyService {
    /// Returns `true` if the named policy is marked as read-only.
    pub fn is_read_only(&self, name: &str) -> bool {
        self.read_only.contains(name)
    }
}

/// Generate a human-readable policy report from a `PolicyService`.
pub fn policy_report(service: &PolicyService) -> Vec<PolicyReportLine> {
    let mut lines: Vec<PolicyReportLine> = service
        .list_policies()
        .iter()
        .filter_map(|name| {
            service.get_policy(name).map(|policy| PolicyReportLine {
                name: name.clone(),
                value: format!("{}", policy.value),
                scope: "default".to_string(),
                read_only: service.is_read_only(name),
            })
        })
        .collect();
    lines.sort_by(|a, b| a.name.cmp(&b.name));
    lines
}

/// Format a full policy report as a multi-line string.
pub fn policy_report_text(service: &PolicyService) -> String {
    let lines = policy_report(service);
    if lines.is_empty() {
        return "No policies configured.".to_string();
    }
    let mut out = String::from("Policy Report\n");
    out.push_str(&"=".repeat(40));
    out.push('\n');
    for line in &lines {
        out.push_str(&format!("{line}\n"));
    }
    out.push_str(&format!("\nTotal: {} policies", lines.len()));
    out
}

/// Summarize policy counts by scope.
pub fn policy_report_by_scope(service: &PolicyService) -> Vec<(String, usize)> {
    let lines = policy_report(service);
    let mut scope_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for line in &lines {
        *scope_counts.entry(line.scope.clone()).or_insert(0) += 1;
    }
    let mut result: Vec<(String, usize)> = scope_counts.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Count read-only vs writable policies.
pub fn policy_report_access_summary(service: &PolicyService) -> (usize, usize) {
    let lines = policy_report(service);
    let ro = lines.iter().filter(|l| l.read_only).count();
    (ro, lines.len() - ro)
}

// ---------------------------------------------------------------------------
// PolicyDiffEntry — diff two PolicyService snapshots
// ---------------------------------------------------------------------------

/// Describes a single difference between two policy snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDiffKind {
    /// Policy exists only in the left snapshot.
    Removed,
    /// Policy exists only in the right snapshot.
    Added,
    /// Policy exists in both but the value changed.
    Changed,
}

/// A single entry in a policy diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDiffEntry {
    pub name: String,
    pub kind: PolicyDiffKind,
    pub old_value: Option<PolicyValue>,
    pub new_value: Option<PolicyValue>,
}

impl fmt::Display for PolicyDiffEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            PolicyDiffKind::Added => {
                write!(f, "+ {}: {}", self.name, self.new_value.as_ref().unwrap())
            }
            PolicyDiffKind::Removed => {
                write!(f, "- {}: {}", self.name, self.old_value.as_ref().unwrap())
            }
            PolicyDiffKind::Changed => {
                write!(
                    f,
                    "~ {}: {} -> {}",
                    self.name,
                    self.old_value.as_ref().unwrap(),
                    self.new_value.as_ref().unwrap()
                )
            }
        }
    }
}

/// Compute the diff between two `PolicyService` instances.
pub fn policy_diff(old: &PolicyService, new: &PolicyService) -> Vec<PolicyDiffEntry> {
    let old_names: HashSet<String> = old.list_policies().into_iter().collect();
    let new_names: HashSet<String> = new.list_policies().into_iter().collect();

    let mut entries = Vec::new();

    for name in &old_names {
        if !new_names.contains(name) {
            entries.push(PolicyDiffEntry {
                name: name.clone(),
                kind: PolicyDiffKind::Removed,
                old_value: old.get_policy(name).map(|p| p.value.clone()),
                new_value: None,
            });
        }
    }

    for name in &new_names {
        if !old_names.contains(name) {
            entries.push(PolicyDiffEntry {
                name: name.clone(),
                kind: PolicyDiffKind::Added,
                old_value: None,
                new_value: new.get_policy(name).map(|p| p.value.clone()),
            });
        } else {
            let ov = &old.get_policy(name).unwrap().value;
            let nv = &new.get_policy(name).unwrap().value;
            if ov != nv {
                entries.push(PolicyDiffEntry {
                    name: name.clone(),
                    kind: PolicyDiffKind::Changed,
                    old_value: Some(ov.clone()),
                    new_value: Some(nv.clone()),
                });
            }
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

// ---------------------------------------------------------------------------
// policy_export_json — minimal JSON serialization without serde
// ---------------------------------------------------------------------------

/// Export all policies in a `PolicyService` as a JSON string.
///
/// This is a lightweight serializer that avoids pulling in serde for a simple
/// flat key→value map.
pub fn policy_export_json(service: &PolicyService) -> String {
    let names = service.list_policies();
    if names.is_empty() {
        return "{}".to_string();
    }
    let mut parts: Vec<String> = Vec::with_capacity(names.len());
    for name in &names {
        if let Some(policy) = service.get_policy(name) {
            let json_val = match &policy.value {
                PolicyValue::Bool(b) => format!("{b}"),
                PolicyValue::Number(n) => format!("{n}"),
                PolicyValue::String(s) => format!("\"{}\"", json_escape(s)),
                PolicyValue::StringList(v) => {
                    let items: Vec<String> =
                        v.iter().map(|s| format!("\"{}\"", json_escape(s))).collect();
                    format!("[{}]", items.join(","))
                }
            };
            parts.push(format!("\"{}\":{}", json_escape(name), json_val));
        }
    }
    format!("{{{}}}", parts.join(","))
}

/// Minimal JSON string escaping.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// PolicyAuditLog — record policy mutation events
// ---------------------------------------------------------------------------

/// The kind of mutation that occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditAction {
    Set,
    Remove,
    MarkReadOnly,
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditAction::Set => write!(f, "SET"),
            AuditAction::Remove => write!(f, "REMOVE"),
            AuditAction::MarkReadOnly => write!(f, "MARK_READ_ONLY"),
        }
    }
}

/// A single entry in the audit log.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub seq: u64,
    pub action: AuditAction,
    pub policy_name: String,
    pub value_before: Option<PolicyValue>,
    pub value_after: Option<PolicyValue>,
}

/// Append-only audit log for policy mutations.
#[derive(Debug)]
pub struct PolicyAuditLog {
    entries: Vec<AuditEntry>,
    next_seq: u64,
}

impl PolicyAuditLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_seq: 1,
        }
    }

    /// Record a policy set/update.
    pub fn record_set(
        &mut self,
        name: &str,
        old: Option<&PolicyValue>,
        new: &PolicyValue,
    ) {
        self.entries.push(AuditEntry {
            seq: self.next_seq,
            action: AuditAction::Set,
            policy_name: name.to_string(),
            value_before: old.cloned(),
            value_after: Some(new.clone()),
        });
        self.next_seq += 1;
    }

    /// Record a policy removal.
    pub fn record_remove(&mut self, name: &str, old: &PolicyValue) {
        self.entries.push(AuditEntry {
            seq: self.next_seq,
            action: AuditAction::Remove,
            policy_name: name.to_string(),
            value_before: Some(old.clone()),
            value_after: None,
        });
        self.next_seq += 1;
    }

    /// Record marking a policy as read-only.
    pub fn record_mark_read_only(&mut self, name: &str) {
        self.entries.push(AuditEntry {
            seq: self.next_seq,
            action: AuditAction::MarkReadOnly,
            policy_name: name.to_string(),
            value_before: None,
            value_after: None,
        });
        self.next_seq += 1;
    }

    /// Return all entries.
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Return the number of recorded entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return entries filtered to a specific policy name.
    pub fn entries_for(&self, name: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.policy_name == name)
            .collect()
    }

    /// Clear the audit log.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for PolicyAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Policy utility functions
// ---------------------------------------------------------------------------

/// Returns the names of all boolean policies that are set to `true`.
pub fn enabled_feature_names(svc: &PolicyService) -> Vec<String> {
    svc.list_policies()
        .into_iter()
        .filter(|name| svc.get_bool(name) == Some(true))
        .collect()
}

/// Returns the names of all boolean policies that are set to `false`.
pub fn disabled_feature_names(svc: &PolicyService) -> Vec<String> {
    svc.list_policies()
        .into_iter()
        .filter(|name| svc.get_bool(name) == Some(false))
        .collect()
}

/// Returns the names of policies whose values are of the `String` variant.
pub fn string_policy_names(svc: &PolicyService) -> Vec<String> {
    svc.list_policies()
        .into_iter()
        .filter(|name| svc.get_string(name).is_some())
        .collect()
}

/// Counts the number of policies grouped by value type.
pub fn count_by_type(svc: &PolicyService) -> HashMap<&'static str, usize> {
    let mut map = HashMap::new();
    for name in svc.list_policies() {
        if let Some(policy) = svc.get_policy(&name) {
            let label = match &policy.value {
                PolicyValue::Bool(_) => "bool",
                PolicyValue::String(_) => "string",
                PolicyValue::Number(_) => "number",
                PolicyValue::StringList(_) => "string_list",
            };
            *map.entry(label).or_insert(0) += 1;
        }
    }
    map
}

/// Returns the names of all policies that match a given prefix.
pub fn policies_with_prefix(svc: &PolicyService, prefix: &str) -> Vec<String> {
    svc.list_policies()
        .into_iter()
        .filter(|n| n.starts_with(prefix))
        .collect()
}

/// Produces a human-readable summary of a `PolicyService`.
pub fn policy_summary(svc: &PolicyService) -> String {
    let total = svc.policy_count();
    let bools = svc
        .list_policies()
        .iter()
        .filter(|n| svc.get_bool(n).is_some())
        .count();
    format!("{total} policies ({bools} boolean)")
}

// ---------------------------------------------------------------------------
// Policy diffing and export utilities
// ---------------------------------------------------------------------------

/// Return the names of policies that changed between two services (added, removed, or modified).
pub fn changed_policy_names(a: &PolicyService, b: &PolicyService) -> Vec<String> {
    let a_names: HashSet<String> = a.list_policies().into_iter().collect();
    let b_names: HashSet<String> = b.list_policies().into_iter().collect();
    let all_names: HashSet<&String> = a_names.iter().chain(b_names.iter()).collect();
    let mut diffs = Vec::new();
    for name in all_names {
        let a_val = a.get_policy(name).map(|p| &p.value);
        let b_val = b.get_policy(name).map(|p| &p.value);
        if a_val != b_val {
            diffs.push(name.clone());
        }
    }
    diffs.sort();
    diffs
}

/// Export all policies as a list of `"name=value"` strings, sorted by name.
pub fn export_policies(svc: &PolicyService) -> Vec<String> {
    let mut pairs: Vec<String> = svc
        .list_policies()
        .into_iter()
        .filter_map(|name| {
            svc.get_policy(&name)
                .map(|p| format!("{}={}", name, p.value))
        })
        .collect();
    pairs.sort();
    pairs
}

/// Return policies grouped by their value type name.
pub fn group_policies_by_type(svc: &PolicyService) -> HashMap<&'static str, Vec<String>> {
    let mut groups: HashMap<&'static str, Vec<String>> = HashMap::new();
    for name in svc.list_policies() {
        if let Some(policy) = svc.get_policy(&name) {
            let label = match &policy.value {
                PolicyValue::Bool(_) => "bool",
                PolicyValue::String(_) => "string",
                PolicyValue::Number(_) => "number",
                PolicyValue::StringList(_) => "string_list",
            };
            groups.entry(label).or_default().push(name);
        }
    }
    groups
}

/// Return true if all boolean policies are set to `true`.
pub fn all_features_enabled(svc: &PolicyService) -> bool {
    svc.list_policies()
        .iter()
        .filter_map(|n| svc.get_bool(n))
        .all(|v| v)
}

/// Return the names of number policies whose values exceed a threshold.
pub fn number_policies_above(svc: &PolicyService, threshold: i64) -> Vec<String> {
    svc.list_policies()
        .into_iter()
        .filter(|n| svc.get_number(n).map_or(false, |v| v > threshold))
        .collect()
}

/// Count the total number of items across all StringList policies.
pub fn total_string_list_items(svc: &PolicyService) -> usize {
    svc.list_policies()
        .iter()
        .filter_map(|n| svc.get_string_list(n))
        .map(|list| list.len())
        .sum()
}

/// Find policy names that violate naming conventions (lowercase, dots, hyphens only).
pub fn find_invalid_policy_names(svc: &PolicyService) -> Vec<String> {
    svc.list_policies()
        .into_iter()
        .filter(|name| {
            !name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '.' || c == '-')
        })
        .collect()
}

impl PolicyService {
    /// Return true if the service contains a policy with the given name.
    pub fn contains_policy(&self, name: &str) -> bool {
        self.get_policy(name).is_some()
    }

    /// Return the names of all boolean policies.
    pub fn bool_policy_names(&self) -> Vec<String> {
        self.list_policies()
            .into_iter()
            .filter(|n| self.get_bool(n).is_some())
            .collect()
    }

    /// Return the names of all string-valued policies.
    pub fn str_policy_names(&self) -> Vec<String> {
        self.list_policies()
            .into_iter()
            .filter(|n| self.get_string(n).is_some())
            .collect()
    }
}

// -- PolicyProfile combining multiple policies -------------------------------

/// A named profile grouping policies together.
#[derive(Debug, Clone)]
pub struct PolicyProfile {
    pub name: String,
    pub policies: Vec<Policy>,
    pub enabled: bool,
}

impl PolicyProfile {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            policies: Vec::new(),
            enabled: true,
        }
    }

    pub fn add_policy(&mut self, policy: Policy) {
        self.policies.push(policy);
    }

    pub fn policy_count(&self) -> usize {
        self.policies.len()
    }

    pub fn get_policy(&self, name: &str) -> Option<&Policy> {
        self.policies.iter().find(|p| p.name == name)
    }

    /// Apply this profile to a PolicyService, overwriting existing policies.
    pub fn apply_to(&self, service: &mut PolicyService) {
        if !self.enabled {
            return;
        }
        for policy in &self.policies {
            service.set_policy(&policy.name, policy.value.clone(), policy.description.clone());
        }
    }
}

impl fmt::Display for PolicyProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.enabled { "enabled" } else { "disabled" };
        write!(f, "Profile({}, {} policies, {})", self.name, self.policies.len(), status)
    }
}

// -- PolicyCheckLog tracking policy checks -----------------------------------

/// A record of a policy check.
#[derive(Debug, Clone)]
pub struct PolicyCheckEntry {
    pub policy_name: String,
    pub action: String,
    pub result: bool,
    pub timestamp: u64,
}

impl fmt::Display for PolicyCheckEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let outcome = if self.result { "allowed" } else { "denied" };
        write!(f, "[{}] {} '{}': {}", self.timestamp, self.action, self.policy_name, outcome)
    }
}

/// Check log for policy evaluations (distinct from the mutation audit log).
#[derive(Debug, Default)]
pub struct PolicyCheckLog {
    entries: Vec<PolicyCheckEntry>,
    max_entries: usize,
}

impl PolicyCheckLog {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Vec::new(), max_entries }
    }

    pub fn record(&mut self, policy_name: &str, action: &str, result: bool, timestamp: u64) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(PolicyCheckEntry {
            policy_name: policy_name.to_string(),
            action: action.to_string(),
            result,
            timestamp,
        });
    }

    pub fn entries(&self) -> &[PolicyCheckEntry] {
        &self.entries
    }

    pub fn denied_entries(&self) -> Vec<&PolicyCheckEntry> {
        self.entries.iter().filter(|e| !e.result).collect()
    }

    pub fn entries_for_policy(&self, name: &str) -> Vec<&PolicyCheckEntry> {
        self.entries.iter().filter(|e| e.policy_name == name).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl fmt::Display for PolicyCheckLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let denied = self.denied_entries().len();
        write!(f, "CheckLog({} entries, {} denied)", self.entries.len(), denied)
    }
}

// -- PolicyOverride for admin bypass -----------------------------------------

/// An override that bypasses a policy for a specific scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyOverride {
    pub policy_name: String,
    pub override_value: PolicyValue,
    pub reason: String,
    pub admin_id: String,
}

impl fmt::Display for PolicyOverride {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Override({}={}, by {})", self.policy_name, self.override_value, self.admin_id)
    }
}

/// Manage policy overrides.
#[derive(Debug, Default)]
pub struct PolicyOverrideManager {
    overrides: Vec<PolicyOverride>,
}

impl PolicyOverrideManager {
    pub fn new() -> Self {
        Self { overrides: Vec::new() }
    }

    pub fn add_override(&mut self, over: PolicyOverride) {
        self.overrides.retain(|o| o.policy_name != over.policy_name);
        self.overrides.push(over);
    }

    pub fn remove_override(&mut self, policy_name: &str) {
        self.overrides.retain(|o| o.policy_name != policy_name);
    }

    pub fn get_override(&self, policy_name: &str) -> Option<&PolicyOverride> {
        self.overrides.iter().find(|o| o.policy_name == policy_name)
    }

    pub fn has_override(&self, policy_name: &str) -> bool {
        self.overrides.iter().any(|o| o.policy_name == policy_name)
    }

    pub fn count(&self) -> usize {
        self.overrides.len()
    }

    /// Resolve a policy value, checking overrides first then the service.
    pub fn resolve(&self, policy_name: &str, service: &PolicyService) -> Option<PolicyValue> {
        if let Some(over) = self.get_override(policy_name) {
            return Some(over.override_value.clone());
        }
        service.get_policy(policy_name).map(|p| p.value.clone())
    }
}

// -- Policy expiration with TTL ----------------------------------------------

/// A policy with an expiration timestamp.
#[derive(Debug, Clone)]
pub struct ExpiringPolicy {
    pub policy: Policy,
    pub expires_at: u64,
}

impl ExpiringPolicy {
    pub fn new(policy: Policy, expires_at: u64) -> Self {
        Self { policy, expires_at }
    }

    pub fn is_expired(&self, now: u64) -> bool {
        now >= self.expires_at
    }

    pub fn remaining(&self, now: u64) -> u64 {
        if now >= self.expires_at { 0 } else { self.expires_at - now }
    }
}

impl fmt::Display for ExpiringPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExpiringPolicy({}, expires_at={})", self.policy.name, self.expires_at)
    }
}

/// Remove expired policies from a list.
pub fn remove_expired(policies: &mut Vec<ExpiringPolicy>, now: u64) -> usize {
    let before = policies.len();
    policies.retain(|p| !p.is_expired(now));
    before - policies.len()
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

    // -- policy_report tests ------------------------------------------------

    fn sample_service() -> PolicyService {
        let mut svc = PolicyService::new();
        svc.set_policy("editor.fontSize", PolicyValue::Number(14), None);
        svc.set_policy("editor.tabSize", PolicyValue::Number(4), None);
        svc.set_policy("security.workspace.trust", PolicyValue::Bool(true), None);
        svc.mark_read_only("security.workspace.trust").unwrap();
        svc.set_policy("telemetry.level", PolicyValue::String("all".into()), None);
        svc
    }

    #[test]
    fn policy_report_lists_all_policies() {
        let svc = sample_service();
        let report = policy_report(&svc);
        assert_eq!(report.len(), 4);
    }

    #[test]
    fn policy_report_sorted_by_name() {
        let svc = sample_service();
        let report = policy_report(&svc);
        let names: Vec<&str> = report.iter().map(|l| l.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn policy_report_text_contains_header() {
        let svc = sample_service();
        let text = policy_report_text(&svc);
        assert!(text.starts_with("Policy Report\n"));
        assert!(text.contains("Total: 4 policies"));
    }

    #[test]
    fn policy_report_text_empty_engine() {
        let svc = PolicyService::new();
        let text = policy_report_text(&svc);
        assert_eq!(text, "No policies configured.");
    }

    #[test]
    fn policy_report_by_scope_counts() {
        let svc = sample_service();
        let scopes = policy_report_by_scope(&svc);
        let default_count = scopes.iter().find(|(s, _)| s == "default").unwrap().1;
        assert_eq!(default_count, 4);
    }

    #[test]
    fn policy_report_access_summary_counts() {
        let svc = sample_service();
        let (ro, rw) = policy_report_access_summary(&svc);
        assert_eq!(ro, 1);
        assert_eq!(rw, 3);
    }

    #[test]
    fn policy_report_line_display() {
        let line = PolicyReportLine {
            name: "test.policy".into(),
            value: "42".into(),
            scope: "user".into(),
            read_only: true,
        };
        let s = format!("{line}");
        assert!(s.contains("[read-only]"));
        assert!(s.contains("test.policy"));
    }

    // -- policy_diff tests --------------------------------------------------

    #[test]
    fn policy_diff_detects_added_removed_changed() {
        let mut old_svc = PolicyService::new();
        old_svc.set_policy("keep", PolicyValue::Number(1), None);
        old_svc.set_policy("change_me", PolicyValue::Bool(true), None);
        old_svc.set_policy("remove_me", PolicyValue::String("bye".into()), None);

        let mut new_svc = PolicyService::new();
        new_svc.set_policy("keep", PolicyValue::Number(1), None);
        new_svc.set_policy("change_me", PolicyValue::Bool(false), None);
        new_svc.set_policy("added", PolicyValue::Number(42), None);

        let diff = policy_diff(&old_svc, &new_svc);
        assert_eq!(diff.len(), 3);

        let added = diff.iter().find(|e| e.name == "added").unwrap();
        assert_eq!(added.kind, PolicyDiffKind::Added);
        assert_eq!(added.new_value, Some(PolicyValue::Number(42)));

        let changed = diff.iter().find(|e| e.name == "change_me").unwrap();
        assert_eq!(changed.kind, PolicyDiffKind::Changed);
        assert_eq!(changed.old_value, Some(PolicyValue::Bool(true)));
        assert_eq!(changed.new_value, Some(PolicyValue::Bool(false)));

        let removed = diff.iter().find(|e| e.name == "remove_me").unwrap();
        assert_eq!(removed.kind, PolicyDiffKind::Removed);
    }

    #[test]
    fn policy_diff_identical_services_empty() {
        let mut a = PolicyService::new();
        a.set_policy("x", PolicyValue::Bool(true), None);
        let mut b = PolicyService::new();
        b.set_policy("x", PolicyValue::Bool(true), None);
        assert!(policy_diff(&a, &b).is_empty());
    }

    #[test]
    fn policy_diff_entry_display() {
        let entry = PolicyDiffEntry {
            name: "telemetry".into(),
            kind: PolicyDiffKind::Changed,
            old_value: Some(PolicyValue::Bool(true)),
            new_value: Some(PolicyValue::Bool(false)),
        };
        let s = format!("{entry}");
        assert!(s.starts_with("~ telemetry:"));
        assert!(s.contains("true"));
        assert!(s.contains("false"));
    }

    // -- policy_export_json tests -------------------------------------------

    #[test]
    fn policy_export_json_empty() {
        let svc = PolicyService::new();
        assert_eq!(policy_export_json(&svc), "{}");
    }

    #[test]
    fn policy_export_json_mixed_types() {
        let mut svc = PolicyService::new();
        svc.set_policy("flag", PolicyValue::Bool(true), None);
        svc.set_policy("count", PolicyValue::Number(7), None);
        svc.set_policy("name", PolicyValue::String("hello".into()), None);
        svc.set_policy(
            "hosts",
            PolicyValue::StringList(vec!["a.com".into(), "b.com".into()]),
            None,
        );

        let json = policy_export_json(&svc);
        assert!(json.contains("\"flag\":true"));
        assert!(json.contains("\"count\":7"));
        assert!(json.contains("\"name\":\"hello\""));
        assert!(json.contains("\"hosts\":[\"a.com\",\"b.com\"]"));
    }

    // -- PolicyAuditLog tests -----------------------------------------------

    #[test]
    fn audit_log_records_and_filters() {
        let mut log = PolicyAuditLog::new();
        assert!(log.is_empty());

        log.record_set("telemetry", None, &PolicyValue::Bool(true));
        log.record_set(
            "telemetry",
            Some(&PolicyValue::Bool(true)),
            &PolicyValue::Bool(false),
        );
        log.record_mark_read_only("telemetry");
        log.record_remove("other", &PolicyValue::Number(1));

        assert_eq!(log.len(), 4);
        assert!(!log.is_empty());

        let tel = log.entries_for("telemetry");
        assert_eq!(tel.len(), 3);
        assert_eq!(tel[0].action, AuditAction::Set);
        assert_eq!(tel[0].seq, 1);
        assert_eq!(tel[2].action, AuditAction::MarkReadOnly);

        let other = log.entries_for("other");
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].action, AuditAction::Remove);
    }

    #[test]
    fn audit_action_display() {
        assert_eq!(AuditAction::Set.to_string(), "SET");
        assert_eq!(AuditAction::Remove.to_string(), "REMOVE");
        assert_eq!(AuditAction::MarkReadOnly.to_string(), "MARK_READ_ONLY");
    }

    // --- new tests ---

    fn sample_policy_svc() -> PolicyService {
        let mut svc = PolicyService::new();
        svc.set_policy("editor.minimap", PolicyValue::Bool(true), None);
        svc.set_policy("editor.wordWrap", PolicyValue::Bool(false), None);
        svc.set_policy("editor.theme", PolicyValue::String("dark".into()), None);
        svc.set_policy("editor.tabSize", PolicyValue::Number(4), None);
        svc.set_policy("terminal.shell", PolicyValue::String("/bin/bash".into()), None);
        svc
    }

    #[test]
    fn test_enabled_feature_names() {
        let svc = sample_policy_svc();
        let enabled = enabled_feature_names(&svc);
        assert_eq!(enabled, vec!["editor.minimap"]);
    }

    #[test]
    fn test_disabled_feature_names() {
        let svc = sample_policy_svc();
        let disabled = disabled_feature_names(&svc);
        assert_eq!(disabled, vec!["editor.wordWrap"]);
    }

    #[test]
    fn test_string_policy_names() {
        let svc = sample_policy_svc();
        let mut names = string_policy_names(&svc);
        names.sort();
        assert_eq!(names, vec!["editor.theme", "terminal.shell"]);
    }

    #[test]
    fn test_count_by_type() {
        let svc = sample_policy_svc();
        let counts = count_by_type(&svc);
        assert_eq!(counts.get("bool"), Some(&2));
        assert_eq!(counts.get("string"), Some(&2));
        assert_eq!(counts.get("number"), Some(&1));
    }

    #[test]
    fn test_policies_with_prefix() {
        let svc = sample_policy_svc();
        let editor = policies_with_prefix(&svc, "editor.");
        assert_eq!(editor.len(), 4);
        let terminal = policies_with_prefix(&svc, "terminal.");
        assert_eq!(terminal.len(), 1);
    }

    #[test]
    fn test_policies_with_prefix_empty() {
        let svc = sample_policy_svc();
        let none = policies_with_prefix(&svc, "nonexistent.");
        assert!(none.is_empty());
    }

    #[test]
    fn test_policy_summary() {
        let svc = sample_policy_svc();
        let summary = policy_summary(&svc);
        assert!(summary.contains("5 policies"));
        assert!(summary.contains("2 boolean"));
    }

    #[test]
    fn test_enabled_feature_names_empty() {
        let svc = PolicyService::new();
        assert!(enabled_feature_names(&svc).is_empty());
    }

    #[test]
    fn changed_policy_names_detects_differences() {
        let mut a = PolicyService::new();
        let mut b = PolicyService::new();
        a.set_policy("shared", PolicyValue::Bool(true), None);
        a.set_policy("only_a", PolicyValue::Number(1), None);
        b.set_policy("shared", PolicyValue::Bool(false), None);
        b.set_policy("only_b", PolicyValue::String("x".into()), None);
        let diffs = changed_policy_names(&a, &b);
        assert_eq!(diffs.len(), 3);
        assert!(diffs.contains(&"shared".to_string()));
        assert!(diffs.contains(&"only_a".to_string()));
        assert!(diffs.contains(&"only_b".to_string()));
    }

    #[test]
    fn changed_policy_names_empty_when_equal() {
        let mut a = PolicyService::new();
        let mut b = PolicyService::new();
        a.set_policy("x", PolicyValue::Bool(true), None);
        b.set_policy("x", PolicyValue::Bool(true), None);
        let diffs = changed_policy_names(&a, &b);
        assert!(diffs.is_empty());
    }

    #[test]
    fn export_policies_format() {
        let mut svc = PolicyService::new();
        svc.set_policy("b.flag", PolicyValue::Bool(true), None);
        svc.set_policy("a.name", PolicyValue::String("val".into()), None);
        let exported = export_policies(&svc);
        assert_eq!(exported[0], "a.name=val");
        assert_eq!(exported[1], "b.flag=true");
    }

    #[test]
    fn group_policies_by_type_groups() {
        let mut svc = PolicyService::new();
        svc.set_policy("flag1", PolicyValue::Bool(true), None);
        svc.set_policy("flag2", PolicyValue::Bool(false), None);
        svc.set_policy("name", PolicyValue::String("x".into()), None);
        let groups = group_policies_by_type(&svc);
        assert_eq!(groups["bool"].len(), 2);
        assert_eq!(groups["string"].len(), 1);
    }

    #[test]
    fn all_features_enabled_checks() {
        let mut svc = PolicyService::new();
        svc.set_policy("a", PolicyValue::Bool(true), None);
        svc.set_policy("b", PolicyValue::Bool(true), None);
        assert!(all_features_enabled(&svc));
        svc.set_policy("c", PolicyValue::Bool(false), None);
        assert!(!all_features_enabled(&svc));
    }

    #[test]
    fn number_policies_above_threshold() {
        let mut svc = PolicyService::new();
        svc.set_policy("low", PolicyValue::Number(5), None);
        svc.set_policy("high", PolicyValue::Number(100), None);
        svc.set_policy("mid", PolicyValue::Number(50), None);
        let above = number_policies_above(&svc, 10);
        assert_eq!(above.len(), 2);
    }

    #[test]
    fn total_string_list_items_sums() {
        let mut svc = PolicyService::new();
        svc.set_policy("list1", PolicyValue::StringList(vec!["a".into(), "b".into()]), None);
        svc.set_policy("list2", PolicyValue::StringList(vec!["c".into()]), None);
        svc.set_policy("flag", PolicyValue::Bool(true), None);
        assert_eq!(total_string_list_items(&svc), 3);
    }

    #[test]
    fn find_invalid_policy_names_catches() {
        let mut svc = PolicyService::new();
        svc.set_policy("valid.name", PolicyValue::Bool(true), None);
        svc.set_policy("INVALID", PolicyValue::Bool(true), None);
        svc.set_policy("also-valid", PolicyValue::Bool(true), None);
        let invalid = find_invalid_policy_names(&svc);
        assert_eq!(invalid.len(), 1);
        assert_eq!(invalid[0], "INVALID");
    }

    #[test]
    fn policy_service_contains_and_names() {
        let mut svc = PolicyService::new();
        svc.set_policy("flag", PolicyValue::Bool(true), None);
        svc.set_policy("name", PolicyValue::String("x".into()), None);
        assert!(svc.contains_policy("flag"));
        assert!(!svc.contains_policy("nonexistent"));
        let bools = svc.bool_policy_names();
        assert_eq!(bools.len(), 1);
        assert!(bools.contains(&"flag".to_string()));
        let strings = svc.str_policy_names();
        assert_eq!(strings.len(), 1);
    }

    // -- PolicyProfile tests --------------------------------------------------

    #[test]
    fn profile_apply_to_service() {
        let mut profile = PolicyProfile::new("strict");
        profile.add_policy(Policy { name: "telemetry".into(), value: PolicyValue::Bool(false), description: None });
        profile.add_policy(Policy { name: "update.mode".into(), value: PolicyValue::String("manual".into()), description: None });

        let mut service = PolicyService::new();
        profile.apply_to(&mut service);
        assert_eq!(service.get_policy("telemetry").unwrap().value, PolicyValue::Bool(false));
        assert_eq!(profile.policy_count(), 2);
    }

    #[test]
    fn profile_disabled_not_applied() {
        let mut profile = PolicyProfile::new("disabled");
        profile.enabled = false;
        profile.add_policy(Policy { name: "x".into(), value: PolicyValue::Bool(true), description: None });

        let mut service = PolicyService::new();
        profile.apply_to(&mut service);
        assert!(service.get_policy("x").is_none());
    }

    #[test]
    fn profile_display() {
        let profile = PolicyProfile::new("test");
        let s = profile.to_string();
        assert!(s.contains("test"));
        assert!(s.contains("enabled"));
    }

    // -- PolicyCheckLog tests -------------------------------------------------

    #[test]
    fn check_log_record_and_query() {
        let mut log = PolicyCheckLog::new(100);
        log.record("telemetry", "check", true, 1000);
        log.record("update", "check", false, 1001);
        assert_eq!(log.len(), 2);
        assert_eq!(log.denied_entries().len(), 1);
    }

    #[test]
    fn check_log_evicts_oldest() {
        let mut log = PolicyCheckLog::new(2);
        log.record("a", "check", true, 1);
        log.record("b", "check", true, 2);
        log.record("c", "check", true, 3);
        assert_eq!(log.len(), 2);
        assert_eq!(log.entries()[0].policy_name, "b");
    }

    #[test]
    fn check_log_for_policy() {
        let mut log = PolicyCheckLog::new(100);
        log.record("x", "check", true, 1);
        log.record("y", "check", false, 2);
        log.record("x", "update", true, 3);
        let x_entries = log.entries_for_policy("x");
        assert_eq!(x_entries.len(), 2);
    }

    #[test]
    fn check_log_display() {
        let log = PolicyCheckLog::new(100);
        let s = log.to_string();
        assert!(s.contains("0 entries"));
    }

    // -- PolicyOverride tests -------------------------------------------------

    #[test]
    fn override_manager_resolve() {
        let mut service = PolicyService::new();
        service.set_policy("flag", PolicyValue::Bool(false), None);

        let mut mgr = PolicyOverrideManager::new();
        mgr.add_override(PolicyOverride {
            policy_name: "flag".into(),
            override_value: PolicyValue::Bool(true),
            reason: "admin bypass".into(),
            admin_id: "admin1".into(),
        });

        let resolved = mgr.resolve("flag", &service);
        assert_eq!(resolved, Some(PolicyValue::Bool(true)));
    }

    #[test]
    fn override_manager_fallback_to_service() {
        let mut service = PolicyService::new();
        service.set_policy("flag", PolicyValue::Bool(false), None);
        let mgr = PolicyOverrideManager::new();
        let resolved = mgr.resolve("flag", &service);
        assert_eq!(resolved, Some(PolicyValue::Bool(false)));
    }

    #[test]
    fn override_replaces_existing() {
        let mut mgr = PolicyOverrideManager::new();
        mgr.add_override(PolicyOverride {
            policy_name: "x".into(), override_value: PolicyValue::Bool(true),
            reason: "r".into(), admin_id: "a".into(),
        });
        mgr.add_override(PolicyOverride {
            policy_name: "x".into(), override_value: PolicyValue::Bool(false),
            reason: "r2".into(), admin_id: "b".into(),
        });
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.get_override("x").unwrap().admin_id, "b");
    }

    // -- ExpiringPolicy tests -------------------------------------------------

    #[test]
    fn expiring_policy_check() {
        let ep = ExpiringPolicy::new(
            Policy { name: "temp".into(), value: PolicyValue::Bool(true), description: None },
            1000,
        );
        assert!(!ep.is_expired(500));
        assert!(ep.is_expired(1000));
        assert_eq!(ep.remaining(500), 500);
        assert_eq!(ep.remaining(1500), 0);
    }

    #[test]
    fn remove_expired_policies() {
        let mut policies = vec![
            ExpiringPolicy::new(Policy { name: "a".into(), value: PolicyValue::Bool(true), description: None }, 100),
            ExpiringPolicy::new(Policy { name: "b".into(), value: PolicyValue::Bool(true), description: None }, 200),
        ];
        let removed = remove_expired(&mut policies, 150);
        assert_eq!(removed, 1);
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].policy.name, "b");
    }

    #[test]
    fn override_display() {
        let o = PolicyOverride {
            policy_name: "x".into(),
            override_value: PolicyValue::Bool(true),
            reason: "test".into(),
            admin_id: "admin".into(),
        };
        let s = o.to_string();
        assert!(s.contains("x"));
        assert!(s.contains("admin"));
    }
}
