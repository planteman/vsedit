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
}
