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

// ---------------------------------------------------------------------------
// PolicyExpressionEvaluator
// ---------------------------------------------------------------------------

/// Token types for policy expression parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyToken {
    Identifier(String),
    And,
    Or,
    Not,
    LeftParen,
    RightParen,
}

impl fmt::Display for PolicyToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyToken::Identifier(s) => write!(f, "{s}"),
            PolicyToken::And => write!(f, "AND"),
            PolicyToken::Or => write!(f, "OR"),
            PolicyToken::Not => write!(f, "NOT"),
            PolicyToken::LeftParen => write!(f, "("),
            PolicyToken::RightParen => write!(f, ")"),
        }
    }
}

/// Tokenizes a policy expression string into tokens.
fn tokenize_policy_expr(input: &str) -> Result<Vec<PolicyToken>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\n' | '\r' => { chars.next(); }
            '(' => { tokens.push(PolicyToken::LeftParen); chars.next(); }
            ')' => { tokens.push(PolicyToken::RightParen); chars.next(); }
            _ if ch.is_alphanumeric() || ch == '_' || ch == '.' => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '.' {
                        word.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                match word.to_uppercase().as_str() {
                    "AND" => tokens.push(PolicyToken::And),
                    "OR" => tokens.push(PolicyToken::Or),
                    "NOT" => tokens.push(PolicyToken::Not),
                    _ => tokens.push(PolicyToken::Identifier(word)),
                }
            }
            other => return Err(format!("unexpected character: '{other}'")),
        }
    }
    Ok(tokens)
}

/// Evaluates boolean policy expressions like "feature.enabled AND NOT trial.expired".
/// Identifiers are resolved against a set of truthy policy keys.
pub struct PolicyExpressionEvaluator {
    truthy_keys: HashSet<String>,
}

impl PolicyExpressionEvaluator {
    pub fn new() -> Self {
        Self { truthy_keys: HashSet::new() }
    }

    pub fn with_keys(keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            truthy_keys: keys.into_iter().map(|k| k.into()).collect(),
        }
    }

    pub fn set_key(&mut self, key: impl Into<String>, value: bool) {
        let k = key.into();
        if value {
            self.truthy_keys.insert(k);
        } else {
            self.truthy_keys.remove(&k);
        }
    }

    pub fn is_key_set(&self, key: &str) -> bool {
        self.truthy_keys.contains(key)
    }

    pub fn key_count(&self) -> usize {
        self.truthy_keys.len()
    }

    pub fn clear(&mut self) {
        self.truthy_keys.clear();
    }

    /// Evaluate a boolean expression string.
    pub fn evaluate(&self, expr: &str) -> Result<bool, String> {
        let tokens = tokenize_policy_expr(expr)?;
        if tokens.is_empty() {
            return Err("empty expression".into());
        }
        let mut pos = 0;
        let result = self.parse_or(&tokens, &mut pos)?;
        if pos < tokens.len() {
            return Err(format!("unexpected token at position {pos}"));
        }
        Ok(result)
    }

    fn parse_or(&self, tokens: &[PolicyToken], pos: &mut usize) -> Result<bool, String> {
        let mut left = self.parse_and(tokens, pos)?;
        while *pos < tokens.len() && tokens[*pos] == PolicyToken::Or {
            *pos += 1;
            let right = self.parse_and(tokens, pos)?;
            left = left || right;
        }
        Ok(left)
    }

    fn parse_and(&self, tokens: &[PolicyToken], pos: &mut usize) -> Result<bool, String> {
        let mut left = self.parse_not(tokens, pos)?;
        while *pos < tokens.len() && tokens[*pos] == PolicyToken::And {
            *pos += 1;
            let right = self.parse_not(tokens, pos)?;
            left = left && right;
        }
        Ok(left)
    }

    fn parse_not(&self, tokens: &[PolicyToken], pos: &mut usize) -> Result<bool, String> {
        if *pos < tokens.len() && tokens[*pos] == PolicyToken::Not {
            *pos += 1;
            let val = self.parse_not(tokens, pos)?;
            return Ok(!val);
        }
        self.parse_primary(tokens, pos)
    }

    fn parse_primary(&self, tokens: &[PolicyToken], pos: &mut usize) -> Result<bool, String> {
        if *pos >= tokens.len() {
            return Err("unexpected end of expression".into());
        }
        match &tokens[*pos] {
            PolicyToken::Identifier(name) => {
                *pos += 1;
                Ok(self.truthy_keys.contains(name))
            }
            PolicyToken::LeftParen => {
                *pos += 1;
                let val = self.parse_or(tokens, pos)?;
                if *pos >= tokens.len() || tokens[*pos] != PolicyToken::RightParen {
                    return Err("missing closing parenthesis".into());
                }
                *pos += 1;
                Ok(val)
            }
            other => Err(format!("unexpected token: {other}")),
        }
    }
}

impl fmt::Display for PolicyExpressionEvaluator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PolicyExpressionEvaluator({} keys)", self.truthy_keys.len())
    }
}

// ---------------------------------------------------------------------------
// PolicyOverrideApplier
// ---------------------------------------------------------------------------

/// A single prioritised policy override entry.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyOverrideEntry {
    pub key: String,
    pub value: String,
    pub priority: i32,
    pub source: String,
}

impl PolicyOverrideEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>, priority: i32, source: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into(), priority, source: source.into() }
    }
}

impl fmt::Display for PolicyOverrideEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={} (pri={}, src={})", self.key, self.value, self.priority, self.source)
    }
}

/// Applies policy overrides with priority ordering.
/// Higher priority overrides win when multiple overrides target the same key.
pub struct PolicyOverrideApplier {
    overrides: Vec<PolicyOverrideEntry>,
}

impl PolicyOverrideApplier {
    pub fn new() -> Self {
        Self { overrides: Vec::new() }
    }

    pub fn add_override(&mut self, entry: PolicyOverrideEntry) {
        self.overrides.push(entry);
    }

    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }

    pub fn clear(&mut self) {
        self.overrides.clear();
    }

    /// Returns the effective value for a key (highest priority override wins).
    pub fn resolve(&self, key: &str) -> Option<&str> {
        self.overrides
            .iter()
            .filter(|e| e.key == key)
            .max_by_key(|e| e.priority)
            .map(|e| e.value.as_str())
    }

    /// Returns all effective overrides as a map (highest priority per key).
    pub fn resolve_all(&self) -> HashMap<String, String> {
        let mut best: HashMap<String, &PolicyOverrideEntry> = HashMap::new();
        for entry in &self.overrides {
            let is_better = best
                .get(&entry.key)
                .map_or(true, |existing| entry.priority > existing.priority);
            if is_better {
                best.insert(entry.key.clone(), entry);
            }
        }
        best.into_iter().map(|(k, v)| (k, v.value.clone())).collect()
    }

    /// Returns all overrides for a specific key, sorted by priority descending.
    pub fn overrides_for_key(&self, key: &str) -> Vec<&PolicyOverrideEntry> {
        let mut entries: Vec<_> = self.overrides.iter().filter(|e| e.key == key).collect();
        entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        entries
    }

    /// Returns the set of all keys that have overrides.
    pub fn overridden_keys(&self) -> HashSet<String> {
        self.overrides.iter().map(|e| e.key.clone()).collect()
    }

    /// Removes all overrides from a given source.
    pub fn remove_source(&mut self, source: &str) {
        self.overrides.retain(|e| e.source != source);
    }

    /// Apply overrides to a base config map, returning the merged result.
    pub fn apply_to(&self, base: &HashMap<String, String>) -> HashMap<String, String> {
        let mut result = base.clone();
        for (k, v) in self.resolve_all() {
            result.insert(k, v);
        }
        result
    }
}

impl fmt::Display for PolicyOverrideApplier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PolicyOverrideApplier({} overrides)", self.overrides.len())
    }
}



// ---------------------------------------------------------------------------
// policy – Data validation and analysis helpers
// ---------------------------------------------------------------------------

/// Result of validating a value against a schema-like rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XPolicyValidationResult {
    Ok,
    Error(String),
    Warning(String),
}

impl XPolicyValidationResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Ok => None,
            Self::Error(m) | Self::Warning(m) => Some(m),
        }
    }
}

/// A key-value pair with optional metadata tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XPolicyTaggedEntry {
    pub key: String,
    pub value: String,
    pub tag: Option<String>,
}

impl XPolicyTaggedEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into(), tag: None }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn matches_tag(&self, tag: &str) -> bool {
        self.tag.as_deref() == Some(tag)
    }
}

/// Validate that a string is non-empty and within a max length.
pub fn x_policy_validate_string(value: &str, max_len: usize) -> XPolicyValidationResult {
    if value.is_empty() {
        return XPolicyValidationResult::Error("value must not be empty".into());
    }
    if value.len() > max_len {
        return XPolicyValidationResult::Error(
            format!("value exceeds max length of {max_len}"),
        );
    }
    XPolicyValidationResult::Ok
}

/// Validate that a number falls within an inclusive range.
pub fn x_policy_validate_range(value: i64, min: i64, max: i64) -> XPolicyValidationResult {
    if value < min || value > max {
        XPolicyValidationResult::Error(
            format!("{value} is outside range [{min}, {max}]"),
        )
    } else {
        XPolicyValidationResult::Ok
    }
}

/// Filter entries by tag, returning only matching ones.
pub fn x_policy_filter_by_tag<'a>(
    entries: &'a [XPolicyTaggedEntry],
    tag: &str,
) -> Vec<&'a XPolicyTaggedEntry> {
    entries.iter().filter(|e| e.matches_tag(tag)).collect()
}

/// Group entries by their tag (entries without a tag go under `"_untagged"`).
pub fn x_policy_group_by_tag(
    entries: &[XPolicyTaggedEntry],
) -> std::collections::HashMap<String, Vec<&XPolicyTaggedEntry>> {
    let mut map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for e in entries {
        let key = e.tag.clone().unwrap_or_else(|| "_untagged".into());
        map.entry(key).or_default().push(e);
    }
    map
}

/// Compute a simple digest of a string (DJB2 hash).
pub fn x_policy_djb2_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

/// Deduplicate entries by key, keeping the first occurrence.
pub fn x_policy_dedup_entries(entries: Vec<XPolicyTaggedEntry>) -> Vec<XPolicyTaggedEntry> {
    let mut seen = std::collections::HashSet::new();
    entries.into_iter().filter(|e| seen.insert(e.key.clone())).collect()
}



// ---------------------------------------------------------------------------
// policy – Extended policy audit log helpers
// ---------------------------------------------------------------------------

/// Priority levels for policy audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZPolicyPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZPolicyPriority {
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
    pub fn all_asc() -> [ZPolicyPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZPolicyPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks policy audit log data.
#[derive(Debug, Clone)]
pub struct ZPolicyPolicyAuditLog {
    pub entries: Vec<(u64, String)>,
    pub max_entries: usize,
    pub sealed: bool,
}

impl ZPolicyPolicyAuditLog {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 0,
            sealed: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZPolicyPolicyAuditLog[max_entries={:?}, sealed={:?}]", self.max_entries, self.sealed)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.sealed = !c.sealed;
        c
    }
}

/// Compute a simple rolling hash for policy audit log.
pub fn z_policy_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_policy_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_policy_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_policy_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_policy_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_policy_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_policy_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 42
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer42 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer42 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_42(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_42<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_42<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_42(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_42(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 140
// ---------------------------------------------------------------------------

/// Generic object pool `Xc140Pool<T>`.
pub struct Xc140Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc140Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc140PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc140Pool<T> {
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
    pub fn stats(&self) -> Xc140PoolStats {
        Xc140PoolStats {
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

impl<T> Default for Xc140Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc140Scheduler`.
pub struct Xc140Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc140Scheduler {
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

impl Default for Xc140Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_140 hash for the given byte slice.
pub fn xc_140_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_140 convention.
pub fn xc_140_reverse(s: &str) -> String {
    s.chars().rev().collect()
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
    fn policy_count_works() {
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

    #[test]
    fn expr_eval_simple_true_key() {
        let eval = PolicyExpressionEvaluator::with_keys(vec!["feature.enabled"]);
        assert!(eval.evaluate("feature.enabled").unwrap());
    }

    #[test]
    fn expr_eval_simple_false_key() {
        let eval = PolicyExpressionEvaluator::new();
        assert!(!eval.evaluate("feature.enabled").unwrap());
    }

    #[test]
    fn expr_eval_and_both_true() {
        let eval = PolicyExpressionEvaluator::with_keys(vec!["a", "b"]);
        assert!(eval.evaluate("a AND b").unwrap());
    }

    #[test]
    fn expr_eval_and_one_false() {
        let eval = PolicyExpressionEvaluator::with_keys(vec!["a"]);
        assert!(!eval.evaluate("a AND b").unwrap());
    }

    #[test]
    fn expr_eval_or_one_true() {
        let eval = PolicyExpressionEvaluator::with_keys(vec!["a"]);
        assert!(eval.evaluate("a OR b").unwrap());
    }

    #[test]
    fn expr_eval_not() {
        let eval = PolicyExpressionEvaluator::new();
        assert!(eval.evaluate("NOT missing").unwrap());
    }

    #[test]
    fn expr_eval_complex_expression() {
        let eval = PolicyExpressionEvaluator::with_keys(vec!["feature.enabled"]);
        assert!(eval.evaluate("feature.enabled AND NOT trial.expired").unwrap());
    }

    #[test]
    fn expr_eval_parentheses() {
        let eval = PolicyExpressionEvaluator::with_keys(vec!["a"]);
        assert!(eval.evaluate("(a OR b) AND NOT c").unwrap());
    }

    #[test]
    fn expr_eval_empty_error() {
        let eval = PolicyExpressionEvaluator::new();
        assert!(eval.evaluate("").is_err());
    }

    #[test]
    fn expr_eval_display_and_key_ops() {
        let mut eval = PolicyExpressionEvaluator::new();
        eval.set_key("x", true);
        assert!(eval.is_key_set("x"));
        assert_eq!(eval.key_count(), 1);
        eval.set_key("x", false);
        assert!(!eval.is_key_set("x"));
        eval.set_key("y", true);
        eval.clear();
        assert_eq!(eval.key_count(), 0);
        assert!(format!("{eval}").contains("0 keys"));
    }

    #[test]
    fn override_applier_resolve_highest_priority() {
        let mut applier = PolicyOverrideApplier::new();
        applier.add_override(PolicyOverrideEntry::new("k", "low", 1, "s1"));
        applier.add_override(PolicyOverrideEntry::new("k", "high", 10, "s2"));
        assert_eq!(applier.resolve("k"), Some("high"));
    }

    #[test]
    fn override_applier_resolve_missing_key() {
        let applier = PolicyOverrideApplier::new();
        assert_eq!(applier.resolve("nope"), None);
    }

    #[test]
    fn override_applier_resolve_all() {
        let mut applier = PolicyOverrideApplier::new();
        applier.add_override(PolicyOverrideEntry::new("a", "1", 5, "s"));
        applier.add_override(PolicyOverrideEntry::new("a", "2", 10, "s"));
        applier.add_override(PolicyOverrideEntry::new("b", "3", 1, "s"));
        let resolved = applier.resolve_all();
        assert_eq!(resolved.get("a").unwrap(), "2");
        assert_eq!(resolved.get("b").unwrap(), "3");
    }

    #[test]
    fn override_applier_overrides_for_key_sorted() {
        let mut applier = PolicyOverrideApplier::new();
        applier.add_override(PolicyOverrideEntry::new("k", "low", 1, "s1"));
        applier.add_override(PolicyOverrideEntry::new("k", "mid", 5, "s2"));
        applier.add_override(PolicyOverrideEntry::new("k", "high", 10, "s3"));
        let entries = applier.overrides_for_key("k");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].value, "high");
    }

    #[test]
    fn override_applier_remove_source() {
        let mut applier = PolicyOverrideApplier::new();
        applier.add_override(PolicyOverrideEntry::new("k", "v1", 1, "admin"));
        applier.add_override(PolicyOverrideEntry::new("k", "v2", 2, "user"));
        applier.remove_source("admin");
        assert_eq!(applier.override_count(), 1);
        assert_eq!(applier.resolve("k"), Some("v2"));
    }

    #[test]
    fn override_applier_apply_to_base() {
        let mut applier = PolicyOverrideApplier::new();
        applier.add_override(PolicyOverrideEntry::new("color", "red", 1, "s"));
        let mut base = HashMap::new();
        base.insert("color".into(), "blue".into());
        base.insert("size".into(), "10".into());
        let merged = applier.apply_to(&base);
        assert_eq!(merged.get("color").unwrap(), "red");
        assert_eq!(merged.get("size").unwrap(), "10");
    }

    #[test]
    fn override_applier_display_and_clear() {
        let mut applier = PolicyOverrideApplier::new();
        applier.add_override(PolicyOverrideEntry::new("k", "v", 1, "s"));
        assert!(format!("{applier}").contains("1 overrides"));
        applier.clear();
        assert_eq!(applier.override_count(), 0);
    }

    #[test]
    fn override_entry_display() {
        let e = PolicyOverrideEntry::new("k", "v", 5, "admin");
        let s = format!("{e}");
        assert!(s.contains("k=v"));
        assert!(s.contains("pri=5"));
        assert!(s.contains("src=admin"));
    }

    #[test]
    fn override_applier_overridden_keys() {
        let mut applier = PolicyOverrideApplier::new();
        applier.add_override(PolicyOverrideEntry::new("a", "1", 1, "s"));
        applier.add_override(PolicyOverrideEntry::new("b", "2", 1, "s"));
        let keys = applier.overridden_keys();
        assert!(keys.contains("a"));
        assert!(keys.contains("b"));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn tokenize_policy_expr_invalid_char() {
        let result = tokenize_policy_expr("a & b");
        assert!(result.is_err());
    }

    #[test]
    fn policy_token_display() {
        assert_eq!(format!("{}", PolicyToken::And), "AND");
        assert_eq!(format!("{}", PolicyToken::Or), "OR");
        assert_eq!(format!("{}", PolicyToken::Not), "NOT");
        assert_eq!(format!("{}", PolicyToken::LeftParen), "(");
        assert_eq!(format!("{}", PolicyToken::RightParen), ")");
        assert_eq!(format!("{}", PolicyToken::Identifier("x".into())), "x");
    }


    // -- policy additional tests -------------------------------------------

    #[test]
    fn x_policy_validation_ok() {
        let r = x_policy_validate_string("hello", 100);
        assert!(r.is_ok());
        assert!(r.message().is_none());
    }

    #[test]
    fn x_policy_validation_empty() {
        let r = x_policy_validate_string("", 100);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("empty"));
    }

    #[test]
    fn x_policy_validation_too_long() {
        let r = x_policy_validate_string("abcdef", 3);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("max length"));
    }

    #[test]
    fn x_policy_validate_range_ok() {
        assert!(x_policy_validate_range(5, 1, 10).is_ok());
        assert!(x_policy_validate_range(1, 1, 10).is_ok());
        assert!(x_policy_validate_range(10, 1, 10).is_ok());
    }

    #[test]
    fn x_policy_validate_range_out() {
        assert!(!x_policy_validate_range(0, 1, 10).is_ok());
        assert!(!x_policy_validate_range(11, 1, 10).is_ok());
    }

    #[test]
    fn x_policy_tagged_entry_basic() {
        let e = XPolicyTaggedEntry::new("k", "v");
        assert_eq!(e.key, "k");
        assert_eq!(e.value, "v");
        assert!(e.tag.is_none());
    }

    #[test]
    fn x_policy_tagged_entry_with_tag() {
        let e = XPolicyTaggedEntry::new("k", "v").with_tag("important");
        assert!(e.matches_tag("important"));
        assert!(!e.matches_tag("other"));
    }

    #[test]
    fn x_policy_filter_by_tag_basic() {
        let entries = vec![
            XPolicyTaggedEntry::new("a", "1").with_tag("x"),
            XPolicyTaggedEntry::new("b", "2").with_tag("y"),
            XPolicyTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let filtered = x_policy_filter_by_tag(&entries, "x");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_policy_group_by_tag_basic() {
        let entries = vec![
            XPolicyTaggedEntry::new("a", "1").with_tag("x"),
            XPolicyTaggedEntry::new("b", "2"),
            XPolicyTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let groups = x_policy_group_by_tag(&entries);
        assert_eq!(groups["x"].len(), 2);
        assert_eq!(groups["_untagged"].len(), 1);
    }

    #[test]
    fn x_policy_djb2_hash_deterministic() {
        let h1 = x_policy_djb2_hash("hello");
        let h2 = x_policy_djb2_hash("hello");
        assert_eq!(h1, h2);
        assert_ne!(x_policy_djb2_hash("hello"), x_policy_djb2_hash("world"));
    }

    #[test]
    fn x_policy_dedup_entries_basic() {
        let entries = vec![
            XPolicyTaggedEntry::new("a", "1"),
            XPolicyTaggedEntry::new("a", "2"),
            XPolicyTaggedEntry::new("b", "3"),
        ];
        let deduped = x_policy_dedup_entries(entries);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].value, "1");
    }

    #[test]
    fn x_policy_validation_result_warning() {
        let w = XPolicyValidationResult::Warning("low disk".into());
        assert!(!w.is_ok());
        assert_eq!(w.message(), Some("low disk"));
    }

    #[test]
    fn x_policy_filter_by_tag_empty() {
        let entries: Vec<XPolicyTaggedEntry> = vec![];
        assert!(x_policy_filter_by_tag(&entries, "x").is_empty());
    }

    #[test]
    fn x_policy_tagged_entry_no_tag_match() {
        let e = XPolicyTaggedEntry::new("k", "v");
        assert!(!e.matches_tag("any"));
    }


    // -- policy Z-extended tests -----------------------------------------------

    #[test]
    fn z_policy_priority_weight() {
        assert_eq!(ZPolicyPriority::Idle.weight(), 0);
        assert_eq!(ZPolicyPriority::Normal.weight(), 2);
        assert_eq!(ZPolicyPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_policy_priority_label() {
        assert_eq!(ZPolicyPriority::Low.label(), "low");
        assert_eq!(ZPolicyPriority::High.label(), "high");
    }

    #[test]
    fn z_policy_priority_is_elevated() {
        assert!(!ZPolicyPriority::Normal.is_elevated());
        assert!(ZPolicyPriority::High.is_elevated());
        assert!(ZPolicyPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_policy_priority_display() {
        assert_eq!(format!("{}", ZPolicyPriority::Idle), "idle");
    }

    #[test]
    fn z_policy_priority_all_asc() {
        let all = ZPolicyPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZPolicyPriority::Idle);
        assert_eq!(all[4], ZPolicyPriority::Realtime);
    }

    #[test]
    fn z_policy_struct_new() {
        let s = ZPolicyPolicyAuditLog::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_policy_struct_toggled_clone() {
        let s = ZPolicyPolicyAuditLog::new();
        let t = s.toggled_clone();
        assert_ne!(s.sealed, t.sealed);
    }

    #[test]
    fn z_policy_rolling_hash_deterministic() {
        let h1 = z_policy_rolling_hash(b"test");
        let h2 = z_policy_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_policy_rolling_hash(b"a"), z_policy_rolling_hash(b"b"));
    }

    #[test]
    fn z_policy_pad_to_basic() {
        assert_eq!(z_policy_pad_to("hi", 5), "hi   ");
        assert_eq!(z_policy_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_policy_is_identifier_basic() {
        assert!(z_policy_is_identifier("foo_bar"));
        assert!(z_policy_is_identifier("abc123"));
        assert!(!z_policy_is_identifier(""));
        assert!(!z_policy_is_identifier("has space"));
    }

    #[test]
    fn z_policy_levenshtein_basic() {
        assert_eq!(z_policy_levenshtein("", ""), 0);
        assert_eq!(z_policy_levenshtein("abc", "abc"), 0);
        assert_eq!(z_policy_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_policy_unique_words_basic() {
        let w = z_policy_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_policy_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_policy_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_policy_common_prefix_basic() {
        assert_eq!(z_policy_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_policy_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_policy_struct_clear() {
        let mut s = ZPolicyPolicyAuditLog::new();
        s.entries.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_policy_rolling_hash_empty() {
        let h = z_policy_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_42_push_and_len() {
        let mut rb = super::XbRingBuffer42::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_42_overwrite() {
        let mut rb = super::XbRingBuffer42::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_42_get_out_of_bounds() {
        let rb = super::XbRingBuffer42::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_42_drain_all() {
        let mut rb = super::XbRingBuffer42::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_42_peek_front_back() {
        let mut rb = super::XbRingBuffer42::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_42_clear() {
        let mut rb = super::XbRingBuffer42::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_42_capacity() {
        let rb = super::XbRingBuffer42::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_42_basic() {
        let h = super::xb_fnv1a_42(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_42(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_42_different_inputs() {
        let h1 = super::xb_fnv1a_42(b"abc");
        let h2 = super::xb_fnv1a_42(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_42_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_42(&data);
        let dec = super::xb_rle_decode_42(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_42_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_42(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_42(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_42_values() {
        assert!((super::xb_clamp_42(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_42(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_42(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_42_values() {
        assert!((super::xb_lerp_42(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_42(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_42(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_42_wrap_around_twice() {
        let mut rb = super::XbRingBuffer42::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 140 ----

    #[test]
    fn xc_140_pool_new_empty() {
        let pool: super::Xc140Pool<i32> = super::Xc140Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_140_pool_release_acquire() {
        let mut pool = super::Xc140Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_140_pool_acquire_empty() {
        let mut pool: super::Xc140Pool<i32> = super::Xc140Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_140_pool_full() {
        let mut pool = super::Xc140Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_140_pool_drain() {
        let mut pool = super::Xc140Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_140_pool_stats() {
        let mut pool = super::Xc140Pool::new(8);
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
    fn xc_140_pool_clear() {
        let mut pool = super::Xc140Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_140_pool_shrink() {
        let mut pool = super::Xc140Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_140_pool_default() {
        let pool: super::Xc140Pool<String> = super::Xc140Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_140_pool_extend() {
        let mut pool = super::Xc140Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_140_pool_retain() {
        let mut pool = super::Xc140Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_140_scheduler_round_robin() {
        let mut sched = super::Xc140Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_140_scheduler_empty() {
        let mut sched = super::Xc140Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_140_scheduler_reset() {
        let mut sched = super::Xc140Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_140_scheduler_add_remove() {
        let mut sched = super::Xc140Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_140_scheduler_targets() {
        let sched = super::Xc140Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_140_hash_empty() {
        assert_eq!(super::xc_140_hash(b""), 5381);
    }

    #[test]
    fn xc_140_hash_data() {
        let h = super::xc_140_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_140_hash(b"hello"), h);
    }

    #[test]
    fn xc_140_reverse_str() {
        assert_eq!(super::xc_140_reverse("abc"), "cba");
        assert_eq!(super::xc_140_reverse(""), "");
    }

}
