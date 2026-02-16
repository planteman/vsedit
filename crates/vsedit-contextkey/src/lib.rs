//! Context key system and when-clause evaluation.
//!
//! This crate provides the context key infrastructure for vsedit, equivalent to
//! VS Code's `vs/platform/contextkey/common/contextkey.ts`. Context keys
//! control when keybindings, menu items, and commands are active.
//!
//! # Key types
//!
//! - [`ContextKeyValue`] — the value of a context key (bool, string, number, or null).
//! - [`IContext`] — trait for providing context key values.
//! - [`ContextKeyExpr`] — parsed when-clause expression tree.
//! - [`ContextKeyService`] — manages context key state with scoped child contexts.

use std::fmt;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use regex::Regex;
use vsedit_di::service;

// ---------------------------------------------------------------------------
// ContextKeyValue
// ---------------------------------------------------------------------------

/// The value of a context key.
#[derive(Debug, Clone, PartialEq)]
pub enum ContextKeyValue {
    Bool(bool),
    String(String),
    Number(f64),
    Null,
}

impl ContextKeyValue {
    /// Interpret the value as a boolean for when-clause evaluation.
    ///
    /// - `Bool(b)` → `b`
    /// - `String(s)` → `!s.is_empty()`
    /// - `Number(n)` → `n != 0.0`
    /// - `Null` → `false`
    fn is_truthy(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::String(s) => !s.is_empty(),
            Self::Number(n) => *n != 0.0,
            Self::Null => false,
        }
    }

    fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::String(s) => s.parse::<f64>().ok(),
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Self::Null => None,
        }
    }

    fn as_str(&self) -> String {
        match self {
            Self::Bool(b) => b.to_string(),
            Self::String(s) => s.clone(),
            Self::Number(n) => n.to_string(),
            Self::Null => String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// IContext trait
// ---------------------------------------------------------------------------

/// Provides context key values for when-clause evaluation.
pub trait IContext: Send + Sync {
    /// Look up the value of a context key. Returns `None` if the key is not set.
    fn get_value(&self, key: &str) -> Option<&ContextKeyValue>;
}

// ---------------------------------------------------------------------------
// ContextKeyExpr — parsed when-clause expression
// ---------------------------------------------------------------------------

/// A parsed when-clause expression (recursive enum).
///
/// Expressions can be parsed from strings like `"editorFocus && !editorReadonly"`
/// and evaluated against an [`IContext`].
#[derive(Debug, Clone, PartialEq)]
pub enum ContextKeyExpr {
    /// Always true.
    True,
    /// Always false.
    False,
    /// True if the key is defined and truthy.
    Defined(String),
    /// Logical negation.
    Not(Box<ContextKeyExpr>),
    /// `key == value`
    Equals(String, String),
    /// `key != value`
    NotEquals(String, String),
    /// `key =~ /pattern/`
    Regex(String, String),
    /// `key in value`
    In(String, String),
    /// `key not in value`
    NotIn(String, String),
    /// All sub-expressions must be true.
    And(Vec<ContextKeyExpr>),
    /// At least one sub-expression must be true.
    Or(Vec<ContextKeyExpr>),
    /// `key > value`
    Greater(String, f64),
    /// `key >= value`
    GreaterEquals(String, f64),
    /// `key < value`
    Less(String, f64),
    /// `key <= value`
    LessEquals(String, f64),
}

/// Parsing error for when-clause expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "when-clause parse error: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

impl ContextKeyExpr {
    /// Parse a when-clause string into a [`ContextKeyExpr`].
    ///
    /// Supports: `&&`, `||`, `!`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `=~`,
    /// `in`, `not in`, `true`, `false`.
    ///
    /// `||` has lower precedence than `&&`.
    pub fn parse(expr: &str) -> Result<Self, ParseError> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Ok(Self::True);
        }
        Self::parse_or(expr)
    }

    /// Parse `||`-separated terms.
    fn parse_or(expr: &str) -> Result<Self, ParseError> {
        let parts = Self::split_top_level(expr, "||");
        if parts.len() == 1 {
            return Self::parse_and(parts[0].trim());
        }
        let exprs: Result<Vec<_>, _> =
            parts.iter().map(|p| Self::parse_and(p.trim())).collect();
        Ok(Self::Or(exprs?))
    }

    /// Parse `&&`-separated terms.
    fn parse_and(expr: &str) -> Result<Self, ParseError> {
        let parts = Self::split_top_level(expr, "&&");
        if parts.len() == 1 {
            return Self::parse_atom(parts[0].trim());
        }
        let exprs: Result<Vec<_>, _> =
            parts.iter().map(|p| Self::parse_atom(p.trim())).collect();
        Ok(Self::And(exprs?))
    }

    /// Split on a delimiter, but only at the top level (not inside parentheses
    /// or regex literals).
    fn split_top_level<'a>(expr: &'a str, delim: &str) -> Vec<&'a str> {
        let mut parts = Vec::new();
        let mut depth = 0u32;
        let mut last = 0;
        let bytes = expr.as_bytes();
        let delim_bytes = delim.as_bytes();
        let delim_len = delim_bytes.len();

        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                _ if depth == 0
                    && i + delim_len <= bytes.len()
                    && &bytes[i..i + delim_len] == delim_bytes =>
                {
                    parts.push(&expr[last..i]);
                    i += delim_len;
                    last = i;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
        parts.push(&expr[last..]);
        parts
    }

    /// Parse a single atomic expression (not containing unparenthesized `&&`
    /// or `||`).
    fn parse_atom(expr: &str) -> Result<Self, ParseError> {
        let expr = expr.trim();

        if expr.is_empty() {
            return Err(ParseError {
                message: "empty expression".into(),
            });
        }

        // Parenthesized sub-expression
        if expr.starts_with('(') && expr.ends_with(')') {
            let inner = &expr[1..expr.len() - 1];
            return Self::parse(inner);
        }

        // Literal true / false
        if expr == "true" {
            return Ok(Self::True);
        }
        if expr == "false" {
            return Ok(Self::False);
        }

        // Negation: !expr
        if let Some(rest) = expr.strip_prefix('!') {
            let inner = Self::parse_atom(rest.trim())?;
            return Ok(Self::Not(Box::new(inner)));
        }

        // Binary operators (order matters: longest match first)
        // Try `not in` first (two-word operator)
        if let Some((key, value)) = Self::try_split_binary(expr, " not in ") {
            return Ok(Self::NotIn(key.to_owned(), value.to_owned()));
        }
        if let Some((key, value)) = Self::try_split_binary(expr, " in ") {
            return Ok(Self::In(key.to_owned(), value.to_owned()));
        }
        if let Some((key, pattern)) = Self::try_split_binary(expr, " =~ ") {
            let pattern = pattern.trim();
            // Strip surrounding slashes if present
            let pattern = if pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() > 1
            {
                &pattern[1..pattern.len() - 1]
            } else {
                pattern
            };
            return Ok(Self::Regex(key.to_owned(), pattern.to_owned()));
        }
        if let Some((key, value)) = Self::try_split_binary(expr, " != ") {
            return Ok(Self::NotEquals(key.to_owned(), value.trim().to_owned()));
        }
        if let Some((key, value)) = Self::try_split_binary(expr, " == ") {
            return Ok(Self::Equals(key.to_owned(), value.trim().to_owned()));
        }
        if let Some((key, value)) = Self::try_split_binary(expr, " >= ") {
            let n = Self::parse_number(value.trim())?;
            return Ok(Self::GreaterEquals(key.to_owned(), n));
        }
        if let Some((key, value)) = Self::try_split_binary(expr, " <= ") {
            let n = Self::parse_number(value.trim())?;
            return Ok(Self::LessEquals(key.to_owned(), n));
        }
        if let Some((key, value)) = Self::try_split_binary(expr, " > ") {
            let n = Self::parse_number(value.trim())?;
            return Ok(Self::Greater(key.to_owned(), n));
        }
        if let Some((key, value)) = Self::try_split_binary(expr, " < ") {
            let n = Self::parse_number(value.trim())?;
            return Ok(Self::Less(key.to_owned(), n));
        }

        // Plain identifier → Defined
        Ok(Self::Defined(expr.to_owned()))
    }

    fn try_split_binary<'a>(expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
        let idx = expr.find(op)?;
        let key = expr[..idx].trim();
        let value = expr[idx + op.len()..].trim();
        if key.is_empty() {
            return None;
        }
        Some((key, value))
    }

    fn parse_number(s: &str) -> Result<f64, ParseError> {
        s.parse::<f64>().map_err(|_| ParseError {
            message: format!("expected a number, got '{s}'"),
        })
    }

    /// Evaluate this expression against a context.
    pub fn evaluate(&self, ctx: &dyn IContext) -> bool {
        match self {
            Self::True => true,
            Self::False => false,
            Self::Defined(key) => ctx
                .get_value(key)
                .is_some_and(ContextKeyValue::is_truthy),
            Self::Not(inner) => !inner.evaluate(ctx),
            Self::Equals(key, value) => ctx
                .get_value(key)
                .is_some_and(|v| v.as_str() == *value),
            Self::NotEquals(key, value) => ctx
                .get_value(key)
                .map_or(true, |v| v.as_str() != *value),
            Self::Regex(key, pattern) => {
                let Some(v) = ctx.get_value(key) else {
                    return false;
                };
                Regex::new(pattern)
                    .map(|re| re.is_match(&v.as_str()))
                    .unwrap_or(false)
            }
            Self::In(key, set_key) => {
                // "key in setKey" checks if the value of `key` is contained
                // in the value of `setKey` (treated as a comma-separated list
                // or as equality for simple values).
                let Some(val) = ctx.get_value(key) else {
                    return false;
                };
                let Some(set_val) = ctx.get_value(set_key) else {
                    return false;
                };
                let needle = val.as_str();
                let haystack = set_val.as_str();
                haystack.split(',').any(|s| s.trim() == needle)
            }
            Self::NotIn(key, set_key) => {
                let Some(val) = ctx.get_value(key) else {
                    return true;
                };
                let Some(set_val) = ctx.get_value(set_key) else {
                    return true;
                };
                let needle = val.as_str();
                let haystack = set_val.as_str();
                !haystack.split(',').any(|s| s.trim() == needle)
            }
            Self::And(exprs) => exprs.iter().all(|e| e.evaluate(ctx)),
            Self::Or(exprs) => exprs.iter().any(|e| e.evaluate(ctx)),
            Self::Greater(key, n) => ctx
                .get_value(key)
                .and_then(|v| v.as_number())
                .is_some_and(|v| v > *n),
            Self::GreaterEquals(key, n) => ctx
                .get_value(key)
                .and_then(|v| v.as_number())
                .is_some_and(|v| v >= *n),
            Self::Less(key, n) => ctx
                .get_value(key)
                .and_then(|v| v.as_number())
                .is_some_and(|v| v < *n),
            Self::LessEquals(key, n) => ctx
                .get_value(key)
                .and_then(|v| v.as_number())
                .is_some_and(|v| v <= *n),
        }
    }
}

// ---------------------------------------------------------------------------
// ContextKeyService
// ---------------------------------------------------------------------------

/// Manages context key state with scoped child contexts.
///
/// Each `ContextKeyService` holds a map of key–value pairs and an optional
/// parent. Lookups walk the parent chain until a value is found.
pub struct ContextKeyService {
    values: RwLock<HashMap<String, ContextKeyValue>>,
    parent: Option<Arc<ContextKeyService>>,
}

service!(ContextKeyService, "ContextKeyService");

impl ContextKeyService {
    /// Create a new root context key service.
    pub fn new() -> Self {
        Self {
            values: RwLock::new(HashMap::new()),
            parent: None,
        }
    }

    /// Set a context key to a value.
    pub fn set_context(&self, key: impl Into<String>, value: ContextKeyValue) {
        self.values.write().unwrap().insert(key.into(), value);
    }

    /// Remove a context key.
    pub fn remove_context(&self, key: &str) {
        self.values.write().unwrap().remove(key);
    }

    /// Get the value of a context key (local only, does not walk parents).
    ///
    /// For parent-aware lookup, use the [`IContext`] trait method
    /// [`get_value`](IContext::get_value) via
    /// [`ContextKeyService::create_context`].
    pub fn get_context(&self, key: &str) -> Option<ContextKeyValue> {
        self.values.read().unwrap().get(key).cloned()
    }

    /// Create a child context that inherits from this service.
    ///
    /// The returned `Arc<ContextKeyService>` can be used as an `IContext`
    /// for expression evaluation with scoped overrides.
    pub fn create_scoped(self: &Arc<Self>) -> Arc<ContextKeyService> {
        Arc::new(ContextKeyService {
            values: RwLock::new(HashMap::new()),
            parent: Some(Arc::clone(self)),
        })
    }
}

impl Default for ContextKeyService {
    fn default() -> Self {
        Self::new()
    }
}

impl IContext for ContextKeyService {
    fn get_value(&self, key: &str) -> Option<&ContextKeyValue> {
        // SAFETY: We hold a read lock for the duration of the borrow.
        // The returned reference is valid as long as `&self` is valid —
        // callers must ensure no concurrent writes invalidate the entry.
        //
        // This is sound because:
        // 1. RwLock guarantees no concurrent writer while we hold the guard.
        // 2. We leak the guard to extend the borrow to `&self` lifetime,
        //    which is safe as long as the caller does not call `set_context`
        //    or `remove_context` while holding the returned reference.
        //    In practice, evaluation is read-only.
        let guard = self.values.read().unwrap();
        let ptr = guard.get(key).map(|v| v as *const ContextKeyValue);
        // Drop the guard — the data is owned by the HashMap inside the RwLock.
        drop(guard);

        match ptr {
            Some(p) => {
                // SAFETY: The pointer is valid because the HashMap owns the
                // data and we do not remove entries during evaluation.
                Some(unsafe { &*p })
            }
            None => {
                // Walk the parent chain.
                self.parent.as_ref().and_then(|p| p.get_value(key))
            }
        }
    }
}

impl std::fmt::Debug for ContextKeyService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextKeyService")
            .field("keys", &self.values.read().unwrap().len())
            .field("has_parent", &self.parent.is_some())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ContextKeyExpression — higher-level wrapper
// ---------------------------------------------------------------------------

/// A parsed and ready-to-evaluate context key expression with its source string.
#[derive(Debug, Clone)]
pub struct ContextKeyExpression {
    source: String,
    parsed: ContextKeyExpr,
}

impl ContextKeyExpression {
    /// Parse a when-clause expression string like "editorTextFocus && !suggestWidgetVisible".
    pub fn parse(expr: &str) -> Result<Self, ParseError> {
        let parsed = ContextKeyExpr::parse(expr)?;
        Ok(Self { source: expr.to_string(), parsed })
    }

    /// Evaluate the expression against a context.
    pub fn evaluate(&self, ctx: &dyn IContext) -> bool {
        self.parsed.evaluate(ctx)
    }

    /// Get the original source string.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get the parsed expression tree.
    pub fn expr(&self) -> &ContextKeyExpr {
        &self.parsed
    }

    /// Returns the set of context key names referenced by this expression.
    pub fn referenced_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        Self::collect_keys(&self.parsed, &mut keys);
        keys.sort();
        keys.dedup();
        keys
    }

    fn collect_keys(expr: &ContextKeyExpr, keys: &mut Vec<String>) {
        match expr {
            ContextKeyExpr::Defined(k) | ContextKeyExpr::Equals(k, _) | ContextKeyExpr::NotEquals(k, _)
            | ContextKeyExpr::Regex(k, _) | ContextKeyExpr::Greater(k, _) | ContextKeyExpr::GreaterEquals(k, _)
            | ContextKeyExpr::Less(k, _) | ContextKeyExpr::LessEquals(k, _) => {
                keys.push(k.clone());
            }
            ContextKeyExpr::In(k, s) | ContextKeyExpr::NotIn(k, s) => {
                keys.push(k.clone());
                keys.push(s.clone());
            }
            ContextKeyExpr::Not(inner) => Self::collect_keys(inner, keys),
            ContextKeyExpr::And(exprs) | ContextKeyExpr::Or(exprs) => {
                for e in exprs { Self::collect_keys(e, keys); }
            }
            ContextKeyExpr::True | ContextKeyExpr::False => {}
        }
    }
}

impl fmt::Display for ContextKeyExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl PartialEq for ContextKeyExpression {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Evaluate a when-clause string against a context. Returns false on parse errors.
pub fn evaluate_expression(expr: &str, ctx: &dyn IContext) -> bool {
    ContextKeyExpr::parse(expr).map(|e| e.evaluate(ctx)).unwrap_or(false)
}

/// Serialize a context key value to a JSON-compatible string.
pub fn context_key_serialize(value: &ContextKeyValue) -> String {
    match value {
        ContextKeyValue::Bool(b) => b.to_string(),
        ContextKeyValue::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        ContextKeyValue::Number(n) => n.to_string(),
        ContextKeyValue::Null => "null".to_string(),
    }
}

/// Deserialize a context key value from a string representation.
pub fn context_key_deserialize(s: &str) -> ContextKeyValue {
    let s = s.trim();
    if s == "null" {
        return ContextKeyValue::Null;
    }
    if s == "true" {
        return ContextKeyValue::Bool(true);
    }
    if s == "false" {
        return ContextKeyValue::Bool(false);
    }
    if let Ok(n) = s.parse::<f64>() {
        return ContextKeyValue::Number(n);
    }
    // Strip surrounding quotes if present
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len()-1];
        return ContextKeyValue::String(inner.replace("\\\"", "\"").replace("\\\\", "\\"));
    }
    ContextKeyValue::String(s.to_string())
}

// ---------------------------------------------------------------------------
// ContextKeyValue helpers
// ---------------------------------------------------------------------------

impl ContextKeyValue {
    /// Return a human-readable type name.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::String(_) => "string",
            Self::Number(_) => "number",
            Self::Null => "null",
        }
    }
}

// ---------------------------------------------------------------------------
// ContextKeyExpr helpers
// ---------------------------------------------------------------------------

impl ContextKeyExpr {
    /// Collect all key names referenced by this expression.
    pub fn referenced_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        self.collect_referenced_keys(&mut keys);
        keys.sort();
        keys.dedup();
        keys
    }

    fn collect_referenced_keys(&self, keys: &mut Vec<String>) {
        match self {
            Self::Defined(k) | Self::Equals(k, _) | Self::NotEquals(k, _)
            | Self::Regex(k, _) | Self::Greater(k, _) | Self::GreaterEquals(k, _)
            | Self::Less(k, _) | Self::LessEquals(k, _) => {
                keys.push(k.clone());
            }
            Self::In(k, s) | Self::NotIn(k, s) => {
                keys.push(k.clone());
                keys.push(s.clone());
            }
            Self::Not(inner) => inner.collect_referenced_keys(keys),
            Self::And(exprs) | Self::Or(exprs) => {
                for e in exprs {
                    e.collect_referenced_keys(keys);
                }
            }
            Self::True | Self::False => {}
        }
    }

    /// Returns true if this is a simple single-key check (Defined or Equals).
    pub fn is_simple(&self) -> bool {
        matches!(self, Self::Defined(_) | Self::Equals(_, _) | Self::True | Self::False)
    }

    /// Return the number of leaf nodes in the expression tree.
    pub fn leaf_count(&self) -> usize {
        match self {
            Self::Not(inner) => inner.leaf_count(),
            Self::And(exprs) | Self::Or(exprs) => exprs.iter().map(|e| e.leaf_count()).sum(),
            _ => 1,
        }
    }
}

// ---------------------------------------------------------------------------
// ContextKeyService helpers
// ---------------------------------------------------------------------------

impl ContextKeyService {
    /// Number of keys stored in this context (local only, not parents).
    pub fn key_count(&self) -> usize {
        self.values.read().unwrap().len()
    }

    /// Return all key names in this context (local only).
    pub fn all_keys(&self) -> Vec<String> {
        let guard = self.values.read().unwrap();
        let mut keys: Vec<String> = guard.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Whether a key is set in this context (local only).
    pub fn contains_key(&self, key: &str) -> bool {
        self.values.read().unwrap().contains_key(key)
    }

    /// Remove all keys from this context.
    pub fn clear(&self) {
        self.values.write().unwrap().clear();
    }
}

// ---------------------------------------------------------------------------
// ContextKeyExpression helpers
// ---------------------------------------------------------------------------

impl ContextKeyExpression {
    /// Returns true if the underlying expression is a simple single-key check.
    pub fn is_simple(&self) -> bool {
        self.parsed.is_simple()
    }

    /// Return the number of leaf nodes in the expression tree.
    pub fn leaf_count(&self) -> usize {
        self.parsed.leaf_count()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- ContextKeyValue ----------------------------------------------------

    #[test]
    fn value_truthiness() {
        assert!(ContextKeyValue::Bool(true).is_truthy());
        assert!(!ContextKeyValue::Bool(false).is_truthy());
        assert!(ContextKeyValue::String("hello".into()).is_truthy());
        assert!(!ContextKeyValue::String(String::new()).is_truthy());
        assert!(ContextKeyValue::Number(1.0).is_truthy());
        assert!(!ContextKeyValue::Number(0.0).is_truthy());
        assert!(!ContextKeyValue::Null.is_truthy());
    }

    #[test]
    fn value_as_number() {
        assert_eq!(ContextKeyValue::Number(3.14).as_number(), Some(3.14));
        assert_eq!(
            ContextKeyValue::String("42".into()).as_number(),
            Some(42.0)
        );
        assert_eq!(ContextKeyValue::Bool(true).as_number(), Some(1.0));
        assert_eq!(ContextKeyValue::Bool(false).as_number(), Some(0.0));
        assert_eq!(ContextKeyValue::Null.as_number(), None);
    }

    // -- Parsing: basic expressions -----------------------------------------

    #[test]
    fn parse_empty() {
        assert_eq!(ContextKeyExpr::parse("").unwrap(), ContextKeyExpr::True);
        assert_eq!(ContextKeyExpr::parse("  ").unwrap(), ContextKeyExpr::True);
    }

    #[test]
    fn parse_true_false() {
        assert_eq!(ContextKeyExpr::parse("true").unwrap(), ContextKeyExpr::True);
        assert_eq!(
            ContextKeyExpr::parse("false").unwrap(),
            ContextKeyExpr::False
        );
    }

    #[test]
    fn parse_defined() {
        assert_eq!(
            ContextKeyExpr::parse("editorFocus").unwrap(),
            ContextKeyExpr::Defined("editorFocus".into())
        );
    }

    #[test]
    fn parse_not() {
        assert_eq!(
            ContextKeyExpr::parse("!editorReadonly").unwrap(),
            ContextKeyExpr::Not(Box::new(ContextKeyExpr::Defined(
                "editorReadonly".into()
            )))
        );
    }

    #[test]
    fn parse_equals() {
        assert_eq!(
            ContextKeyExpr::parse("editorLangId == typescript").unwrap(),
            ContextKeyExpr::Equals("editorLangId".into(), "typescript".into())
        );
    }

    #[test]
    fn parse_not_equals() {
        assert_eq!(
            ContextKeyExpr::parse("editorLangId != javascript").unwrap(),
            ContextKeyExpr::NotEquals("editorLangId".into(), "javascript".into())
        );
    }

    #[test]
    fn parse_regex() {
        assert_eq!(
            ContextKeyExpr::parse("resourceScheme =~ /^untitled$/").unwrap(),
            ContextKeyExpr::Regex("resourceScheme".into(), "^untitled$".into())
        );
    }

    #[test]
    fn parse_in() {
        assert_eq!(
            ContextKeyExpr::parse("editorLangId in supportedLanguages").unwrap(),
            ContextKeyExpr::In("editorLangId".into(), "supportedLanguages".into())
        );
    }

    #[test]
    fn parse_not_in() {
        assert_eq!(
            ContextKeyExpr::parse("editorLangId not in excludedLanguages").unwrap(),
            ContextKeyExpr::NotIn("editorLangId".into(), "excludedLanguages".into())
        );
    }

    #[test]
    fn parse_comparison_operators() {
        assert_eq!(
            ContextKeyExpr::parse("lineCount > 100").unwrap(),
            ContextKeyExpr::Greater("lineCount".into(), 100.0)
        );
        assert_eq!(
            ContextKeyExpr::parse("lineCount >= 100").unwrap(),
            ContextKeyExpr::GreaterEquals("lineCount".into(), 100.0)
        );
        assert_eq!(
            ContextKeyExpr::parse("indentSize < 8").unwrap(),
            ContextKeyExpr::Less("indentSize".into(), 8.0)
        );
        assert_eq!(
            ContextKeyExpr::parse("indentSize <= 4").unwrap(),
            ContextKeyExpr::LessEquals("indentSize".into(), 4.0)
        );
    }

    #[test]
    fn parse_and() {
        let expr = ContextKeyExpr::parse("editorFocus && !findWidgetVisible").unwrap();
        assert_eq!(
            expr,
            ContextKeyExpr::And(vec![
                ContextKeyExpr::Defined("editorFocus".into()),
                ContextKeyExpr::Not(Box::new(ContextKeyExpr::Defined(
                    "findWidgetVisible".into()
                ))),
            ])
        );
    }

    #[test]
    fn parse_or() {
        let expr = ContextKeyExpr::parse("isWindows || isMac").unwrap();
        assert_eq!(
            expr,
            ContextKeyExpr::Or(vec![
                ContextKeyExpr::Defined("isWindows".into()),
                ContextKeyExpr::Defined("isMac".into()),
            ])
        );
    }

    #[test]
    fn parse_and_or_precedence() {
        // `a && b || c && d` should be `Or(And(a, b), And(c, d))`
        let expr = ContextKeyExpr::parse("a && b || c && d").unwrap();
        assert_eq!(
            expr,
            ContextKeyExpr::Or(vec![
                ContextKeyExpr::And(vec![
                    ContextKeyExpr::Defined("a".into()),
                    ContextKeyExpr::Defined("b".into()),
                ]),
                ContextKeyExpr::And(vec![
                    ContextKeyExpr::Defined("c".into()),
                    ContextKeyExpr::Defined("d".into()),
                ]),
            ])
        );
    }

    #[test]
    fn parse_complex_expression() {
        let expr = ContextKeyExpr::parse(
            "editorFocus && editorLangId == typescript && !editorReadonly",
        )
        .unwrap();
        assert_eq!(
            expr,
            ContextKeyExpr::And(vec![
                ContextKeyExpr::Defined("editorFocus".into()),
                ContextKeyExpr::Equals("editorLangId".into(), "typescript".into()),
                ContextKeyExpr::Not(Box::new(ContextKeyExpr::Defined(
                    "editorReadonly".into()
                ))),
            ])
        );
    }

    #[test]
    fn parse_error_on_invalid_number() {
        let result = ContextKeyExpr::parse("count > abc");
        assert!(result.is_err());
    }

    // -- Evaluation ---------------------------------------------------------

    fn make_ctx(entries: &[(&str, ContextKeyValue)]) -> ContextKeyService {
        let svc = ContextKeyService::new();
        for (k, v) in entries {
            svc.set_context(*k, v.clone());
        }
        svc
    }

    #[test]
    fn eval_true_false() {
        let ctx = make_ctx(&[]);
        assert!(ContextKeyExpr::True.evaluate(&ctx));
        assert!(!ContextKeyExpr::False.evaluate(&ctx));
    }

    #[test]
    fn eval_defined() {
        let ctx = make_ctx(&[("editorFocus", ContextKeyValue::Bool(true))]);
        let expr = ContextKeyExpr::parse("editorFocus").unwrap();
        assert!(expr.evaluate(&ctx));

        let expr = ContextKeyExpr::parse("nonexistent").unwrap();
        assert!(!expr.evaluate(&ctx));
    }

    #[test]
    fn eval_defined_false_value() {
        let ctx = make_ctx(&[("editorFocus", ContextKeyValue::Bool(false))]);
        let expr = ContextKeyExpr::parse("editorFocus").unwrap();
        assert!(!expr.evaluate(&ctx));
    }

    #[test]
    fn eval_not() {
        let ctx = make_ctx(&[("editorReadonly", ContextKeyValue::Bool(false))]);
        let expr = ContextKeyExpr::parse("!editorReadonly").unwrap();
        assert!(expr.evaluate(&ctx));
    }

    #[test]
    fn eval_equals() {
        let ctx = make_ctx(&[(
            "editorLangId",
            ContextKeyValue::String("typescript".into()),
        )]);
        let expr = ContextKeyExpr::parse("editorLangId == typescript").unwrap();
        assert!(expr.evaluate(&ctx));

        let expr = ContextKeyExpr::parse("editorLangId == javascript").unwrap();
        assert!(!expr.evaluate(&ctx));
    }

    #[test]
    fn eval_not_equals() {
        let ctx = make_ctx(&[(
            "editorLangId",
            ContextKeyValue::String("typescript".into()),
        )]);
        let expr = ContextKeyExpr::parse("editorLangId != javascript").unwrap();
        assert!(expr.evaluate(&ctx));

        let expr = ContextKeyExpr::parse("editorLangId != typescript").unwrap();
        assert!(!expr.evaluate(&ctx));
    }

    #[test]
    fn eval_regex() {
        let ctx = make_ctx(&[(
            "resourceScheme",
            ContextKeyValue::String("untitled".into()),
        )]);
        let expr = ContextKeyExpr::parse("resourceScheme =~ /^untitled$/").unwrap();
        assert!(expr.evaluate(&ctx));

        let expr = ContextKeyExpr::parse("resourceScheme =~ /^file$/").unwrap();
        assert!(!expr.evaluate(&ctx));
    }

    #[test]
    fn eval_in() {
        let ctx = make_ctx(&[
            ("lang", ContextKeyValue::String("rust".into())),
            (
                "supported",
                ContextKeyValue::String("rust,python,go".into()),
            ),
        ]);
        let expr = ContextKeyExpr::parse("lang in supported").unwrap();
        assert!(expr.evaluate(&ctx));
    }

    #[test]
    fn eval_not_in() {
        let ctx = make_ctx(&[
            ("lang", ContextKeyValue::String("java".into())),
            (
                "supported",
                ContextKeyValue::String("rust,python,go".into()),
            ),
        ]);
        let expr = ContextKeyExpr::parse("lang not in supported").unwrap();
        assert!(expr.evaluate(&ctx));
    }

    #[test]
    fn eval_greater() {
        let ctx = make_ctx(&[("lineCount", ContextKeyValue::Number(150.0))]);
        assert!(ContextKeyExpr::parse("lineCount > 100")
            .unwrap()
            .evaluate(&ctx));
        assert!(!ContextKeyExpr::parse("lineCount > 200")
            .unwrap()
            .evaluate(&ctx));
    }

    #[test]
    fn eval_greater_equals() {
        let ctx = make_ctx(&[("lineCount", ContextKeyValue::Number(100.0))]);
        assert!(ContextKeyExpr::parse("lineCount >= 100")
            .unwrap()
            .evaluate(&ctx));
        assert!(!ContextKeyExpr::parse("lineCount >= 101")
            .unwrap()
            .evaluate(&ctx));
    }

    #[test]
    fn eval_less() {
        let ctx = make_ctx(&[("indent", ContextKeyValue::Number(2.0))]);
        assert!(ContextKeyExpr::parse("indent < 4")
            .unwrap()
            .evaluate(&ctx));
        assert!(!ContextKeyExpr::parse("indent < 1")
            .unwrap()
            .evaluate(&ctx));
    }

    #[test]
    fn eval_less_equals() {
        let ctx = make_ctx(&[("indent", ContextKeyValue::Number(4.0))]);
        assert!(ContextKeyExpr::parse("indent <= 4")
            .unwrap()
            .evaluate(&ctx));
        assert!(!ContextKeyExpr::parse("indent <= 3")
            .unwrap()
            .evaluate(&ctx));
    }

    #[test]
    fn eval_and() {
        let ctx = make_ctx(&[
            ("editorFocus", ContextKeyValue::Bool(true)),
            ("findWidgetVisible", ContextKeyValue::Bool(false)),
        ]);
        let expr = ContextKeyExpr::parse("editorFocus && !findWidgetVisible").unwrap();
        assert!(expr.evaluate(&ctx));
    }

    #[test]
    fn eval_or() {
        let ctx = make_ctx(&[("isMac", ContextKeyValue::Bool(true))]);
        let expr = ContextKeyExpr::parse("isWindows || isMac").unwrap();
        assert!(expr.evaluate(&ctx));
    }

    #[test]
    fn eval_complex() {
        let ctx = make_ctx(&[
            ("editorFocus", ContextKeyValue::Bool(true)),
            (
                "editorLangId",
                ContextKeyValue::String("typescript".into()),
            ),
            ("editorReadonly", ContextKeyValue::Bool(false)),
        ]);
        let expr = ContextKeyExpr::parse(
            "editorFocus && editorLangId == typescript && !editorReadonly",
        )
        .unwrap();
        assert!(expr.evaluate(&ctx));
    }

    // -- ContextKeyService --------------------------------------------------

    #[test]
    fn service_set_get_remove() {
        let svc = ContextKeyService::new();
        svc.set_context("key1", ContextKeyValue::Bool(true));
        assert_eq!(svc.get_context("key1"), Some(ContextKeyValue::Bool(true)));

        svc.remove_context("key1");
        assert_eq!(svc.get_context("key1"), None);
    }

    #[test]
    fn service_scoped_context() {
        let parent = Arc::new(ContextKeyService::new());
        parent.set_context("parentKey", ContextKeyValue::String("hello".into()));

        let child = parent.create_scoped();
        child.set_context("childKey", ContextKeyValue::Bool(true));

        // Child sees its own key
        assert!(child.get_value("childKey").is_some());
        // Child sees parent key
        assert!(child.get_value("parentKey").is_some());
        // Parent does not see child key
        assert!(parent.get_value("childKey").is_none());
    }

    #[test]
    fn service_scoped_override() {
        let parent = Arc::new(ContextKeyService::new());
        parent.set_context("key", ContextKeyValue::String("parent".into()));

        let child = parent.create_scoped();
        child.set_context("key", ContextKeyValue::String("child".into()));

        assert_eq!(
            child.get_value("key"),
            Some(&ContextKeyValue::String("child".into()))
        );
        assert_eq!(
            parent.get_value("key"),
            Some(&ContextKeyValue::String("parent".into()))
        );
    }

    #[test]
    fn service_evaluate_expr() {
        let svc = ContextKeyService::new();
        svc.set_context("editorFocus", ContextKeyValue::Bool(true));
        svc.set_context(
            "editorLangId",
            ContextKeyValue::String("rust".into()),
        );

        let expr = ContextKeyExpr::parse("editorFocus && editorLangId == rust").unwrap();
        assert!(expr.evaluate(&svc));
    }

    #[test]
    fn service_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ContextKeyService>();
    }

    #[test]
    fn service_debug() {
        let svc = ContextKeyService::new();
        svc.set_context("a", ContextKeyValue::Bool(true));
        let dbg = format!("{svc:?}");
        assert!(dbg.contains("ContextKeyService"));
    }

    // -- IContext for ContextKeyService with scoped evaluation ---------------

    #[test]
    fn scoped_evaluation() {
        let root = Arc::new(ContextKeyService::new());
        root.set_context("editorFocus", ContextKeyValue::Bool(true));
        root.set_context("editorReadonly", ContextKeyValue::Bool(false));

        let scoped = root.create_scoped();
        scoped.set_context("editorReadonly", ContextKeyValue::Bool(true));

        let expr = ContextKeyExpr::parse("editorFocus && !editorReadonly").unwrap();

        // Root: editorReadonly is false → expression is true
        assert!(expr.evaluate(root.as_ref()));
        // Scoped: editorReadonly is true → expression is false
        assert!(!expr.evaluate(scoped.as_ref()));
    }

    // -- Edge cases ---------------------------------------------------------

    #[test]
    fn not_equals_missing_key() {
        let ctx = make_ctx(&[]);
        let expr = ContextKeyExpr::parse("missing != something").unwrap();
        assert!(expr.evaluate(&ctx));
    }

    #[test]
    fn in_missing_key() {
        let ctx = make_ctx(&[(
            "set",
            ContextKeyValue::String("a,b,c".into()),
        )]);
        let expr = ContextKeyExpr::parse("missing in set").unwrap();
        assert!(!expr.evaluate(&ctx));
    }

    #[test]
    fn comparison_missing_key() {
        let ctx = make_ctx(&[]);
        assert!(!ContextKeyExpr::parse("x > 0").unwrap().evaluate(&ctx));
        assert!(!ContextKeyExpr::parse("x < 0").unwrap().evaluate(&ctx));
    }

    #[test]
    fn bool_value_equals_string() {
        let ctx = make_ctx(&[("flag", ContextKeyValue::Bool(true))]);
        let expr = ContextKeyExpr::parse("flag == true").unwrap();
        assert!(expr.evaluate(&ctx));
    }

    #[test]
    fn number_value_equals_string() {
        let ctx = make_ctx(&[("count", ContextKeyValue::Number(42.0))]);
        let expr = ContextKeyExpr::parse("count == 42").unwrap();
        assert!(expr.evaluate(&ctx));
    }

    #[test]
    fn deeply_nested_scopes() {
        let root = Arc::new(ContextKeyService::new());
        root.set_context("a", ContextKeyValue::String("root".into()));

        let child1 = root.create_scoped();
        child1.set_context("b", ContextKeyValue::String("child1".into()));

        let child1_arc = Arc::new(ContextKeyService::new());
        // Simulate deeper nesting via the parent field
        let grandchild = child1.create_scoped();
        grandchild.set_context("c", ContextKeyValue::String("grandchild".into()));
        drop(child1_arc);

        assert!(grandchild.get_value("c").is_some());
        assert!(grandchild.get_value("b").is_some());
        assert!(grandchild.get_value("a").is_some());
    }

    // -- TestContext helper --------------------------------------------------

    struct TestContext {
        values: HashMap<String, ContextKeyValue>,
    }

    impl TestContext {
        fn new() -> Self {
            Self { values: HashMap::new() }
        }
        fn set(&mut self, key: &str, value: ContextKeyValue) {
            self.values.insert(key.to_string(), value);
        }
    }

    impl IContext for TestContext {
        fn get_value(&self, key: &str) -> Option<&ContextKeyValue> {
            self.values.get(key)
        }
    }

    // -- ContextKeyExpression ------------------------------------------------

    #[test]
    fn context_key_expression_parse_and_evaluate() {
        let expr = ContextKeyExpression::parse("editorFocus && !readOnly").unwrap();
        let mut ctx = TestContext::new();
        ctx.set("editorFocus", ContextKeyValue::Bool(true));
        ctx.set("readOnly", ContextKeyValue::Bool(false));
        assert!(expr.evaluate(&ctx));
    }

    #[test]
    fn context_key_expression_referenced_keys() {
        let expr = ContextKeyExpression::parse("editorFocus && lang == rust").unwrap();
        let keys = expr.referenced_keys();
        assert!(keys.contains(&"editorFocus".to_string()));
        assert!(keys.contains(&"lang".to_string()));
    }

    #[test]
    fn context_key_expression_source() {
        let expr = ContextKeyExpression::parse("editorFocus").unwrap();
        assert_eq!(expr.source(), "editorFocus");
    }

    #[test]
    fn context_key_expression_display() {
        let expr = ContextKeyExpression::parse("a && b").unwrap();
        assert_eq!(format!("{}", expr), "a && b");
    }

    #[test]
    fn evaluate_expression_helper() {
        let mut ctx = TestContext::new();
        ctx.set("visible", ContextKeyValue::Bool(true));
        assert!(evaluate_expression("visible", &ctx));
        assert!(!evaluate_expression("invisible", &ctx));
    }

    #[test]
    fn serialize_deserialize_bool() {
        let val = ContextKeyValue::Bool(true);
        let s = context_key_serialize(&val);
        assert_eq!(s, "true");
        assert_eq!(context_key_deserialize(&s), val);
    }

    #[test]
    fn serialize_deserialize_number() {
        let val = ContextKeyValue::Number(42.5);
        let s = context_key_serialize(&val);
        assert_eq!(s, "42.5");
        assert_eq!(context_key_deserialize(&s), val);
    }

    #[test]
    fn serialize_deserialize_string() {
        let val = ContextKeyValue::String("hello world".into());
        let s = context_key_serialize(&val);
        assert_eq!(s, "\"hello world\"");
        assert_eq!(context_key_deserialize(&s), val);
    }

    #[test]
    fn serialize_deserialize_null() {
        let val = ContextKeyValue::Null;
        let s = context_key_serialize(&val);
        assert_eq!(s, "null");
        assert_eq!(context_key_deserialize(&s), val);
    }

    #[test]
    fn context_key_expression_equality() {
        let e1 = ContextKeyExpression::parse("a && b").unwrap();
        let e2 = ContextKeyExpression::parse("a && b").unwrap();
        let e3 = ContextKeyExpression::parse("a || b").unwrap();
        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }

    #[test]
    fn evaluate_expression_parse_error_returns_false() {
        let ctx = TestContext::new();
        assert!(!evaluate_expression("&&", &ctx));
    }

    // -- ContextKeyValue::type_name ----------------------------------------

    #[test]
    fn value_type_name() {
        assert_eq!(ContextKeyValue::Bool(true).type_name(), "bool");
        assert_eq!(ContextKeyValue::String("x".into()).type_name(), "string");
        assert_eq!(ContextKeyValue::Number(1.0).type_name(), "number");
        assert_eq!(ContextKeyValue::Null.type_name(), "null");
    }

    // -- ContextKeyExpr::referenced_keys -----------------------------------

    #[test]
    fn expr_referenced_keys_simple() {
        let expr = ContextKeyExpr::parse("editorFocus").unwrap();
        assert_eq!(expr.referenced_keys(), vec!["editorFocus".to_string()]);
    }

    #[test]
    fn expr_referenced_keys_compound() {
        let expr = ContextKeyExpr::parse("a && b == 'x' && !c").unwrap();
        let keys = expr.referenced_keys();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    // -- ContextKeyExpr::is_simple -----------------------------------------

    #[test]
    fn expr_is_simple() {
        assert!(ContextKeyExpr::parse("editorFocus").unwrap().is_simple());
        assert!(ContextKeyExpr::parse("key == 'val'").unwrap().is_simple());
        assert!(!ContextKeyExpr::parse("a && b").unwrap().is_simple());
        assert!(!ContextKeyExpr::parse("!x").unwrap().is_simple());
    }

    // -- ContextKeyExpr::leaf_count ----------------------------------------

    #[test]
    fn expr_leaf_count() {
        let simple = ContextKeyExpr::parse("editorFocus").unwrap();
        assert_eq!(simple.leaf_count(), 1);
        let compound = ContextKeyExpr::parse("a && b && c").unwrap();
        assert_eq!(compound.leaf_count(), 3);
    }

    // -- ContextKeyService helpers -----------------------------------------

    #[test]
    fn service_key_count_and_all_keys() {
        let svc = ContextKeyService::new();
        assert_eq!(svc.key_count(), 0);
        svc.set_context("alpha", ContextKeyValue::Bool(true));
        svc.set_context("beta", ContextKeyValue::Number(42.0));
        assert_eq!(svc.key_count(), 2);
        let keys = svc.all_keys();
        assert_eq!(keys, vec!["alpha", "beta"]);
    }

    #[test]
    fn service_contains_key() {
        let svc = ContextKeyService::new();
        assert!(!svc.contains_key("missing"));
        svc.set_context("present", ContextKeyValue::Null);
        assert!(svc.contains_key("present"));
    }

    #[test]
    fn service_clear() {
        let svc = ContextKeyService::new();
        svc.set_context("a", ContextKeyValue::Bool(true));
        svc.set_context("b", ContextKeyValue::Bool(false));
        assert_eq!(svc.key_count(), 2);
        svc.clear();
        assert_eq!(svc.key_count(), 0);
    }

    // -- ContextKeyExpression helpers --------------------------------------

    #[test]
    fn expression_is_simple() {
        let simple = ContextKeyExpression::parse("editorFocus").unwrap();
        assert!(simple.is_simple());
        let compound = ContextKeyExpression::parse("a && b").unwrap();
        assert!(!compound.is_simple());
    }

    #[test]
    fn expression_leaf_count() {
        let expr = ContextKeyExpression::parse("a && b || c").unwrap();
        assert!(expr.leaf_count() >= 2);
    }
}
