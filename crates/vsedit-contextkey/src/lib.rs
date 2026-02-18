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
// Display for ContextKeyValue
// ---------------------------------------------------------------------------

impl fmt::Display for ContextKeyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Number(n) => write!(f, "{n}"),
            Self::Null => write!(f, "null"),
        }
    }
}

// ---------------------------------------------------------------------------
// From impls for ContextKeyValue
// ---------------------------------------------------------------------------

impl From<bool> for ContextKeyValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<f64> for ContextKeyValue {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

impl From<String> for ContextKeyValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for ContextKeyValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_owned())
    }
}

// ---------------------------------------------------------------------------
// ContextKeySnapshot
// ---------------------------------------------------------------------------

/// A frozen snapshot of all context keys at a point in time.
///
/// Useful for debugging, logging, or comparing context state across operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextKeySnapshot {
    entries: HashMap<String, ContextKeyValue>,
}

impl ContextKeySnapshot {
    /// Capture the current state of a [`ContextKeyService`].
    pub fn capture(service: &ContextKeyService) -> Self {
        let guard = service.values.read().unwrap();
        Self {
            entries: guard.clone(),
        }
    }

    /// Create a snapshot from an iterator of key-value pairs.
    pub fn from_entries(iter: impl IntoIterator<Item = (String, ContextKeyValue)>) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }

    /// Number of keys in the snapshot.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up a value in the snapshot.
    pub fn get(&self, key: &str) -> Option<&ContextKeyValue> {
        self.entries.get(key)
    }

    /// All key names in sorted order.
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.entries.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Compute the difference between this snapshot and another.
    pub fn diff(&self, other: &ContextKeySnapshot) -> ContextKeyDiff {
        ContextKeyDiff::compute(self, other)
    }
}

impl fmt::Display for ContextKeySnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut keys: Vec<_> = self.entries.keys().collect();
        keys.sort();
        writeln!(f, "ContextKeySnapshot ({} keys):", keys.len())?;
        for key in keys {
            let val = &self.entries[key];
            writeln!(f, "  {key} = {val}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ContextKeyDiff
// ---------------------------------------------------------------------------

/// Represents a single changed key between two snapshots.
#[derive(Debug, Clone, PartialEq)]
pub enum ContextKeyChange {
    /// Key was added (not in old, present in new).
    Added(String, ContextKeyValue),
    /// Key was removed (present in old, not in new).
    Removed(String, ContextKeyValue),
    /// Key value changed.
    Changed(String, ContextKeyValue, ContextKeyValue),
}

/// The difference between two [`ContextKeySnapshot`]s.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextKeyDiff {
    pub changes: Vec<ContextKeyChange>,
}

impl ContextKeyDiff {
    /// Compute the diff from `old` to `new`.
    pub fn compute(old: &ContextKeySnapshot, new: &ContextKeySnapshot) -> Self {
        let mut changes = Vec::new();

        // Keys removed or changed
        for (key, old_val) in &old.entries {
            match new.entries.get(key) {
                None => changes.push(ContextKeyChange::Removed(key.clone(), old_val.clone())),
                Some(new_val) if new_val != old_val => {
                    changes.push(ContextKeyChange::Changed(
                        key.clone(),
                        old_val.clone(),
                        new_val.clone(),
                    ));
                }
                _ => {}
            }
        }

        // Keys added
        for (key, new_val) in &new.entries {
            if !old.entries.contains_key(key) {
                changes.push(ContextKeyChange::Added(key.clone(), new_val.clone()));
            }
        }

        changes.sort_by(|a, b| {
            let ka = match a {
                ContextKeyChange::Added(k, _)
                | ContextKeyChange::Removed(k, _)
                | ContextKeyChange::Changed(k, _, _) => k,
            };
            let kb = match b {
                ContextKeyChange::Added(k, _)
                | ContextKeyChange::Removed(k, _)
                | ContextKeyChange::Changed(k, _, _) => k,
            };
            ka.cmp(kb)
        });

        Self { changes }
    }

    /// Whether the two snapshots are identical (no changes).
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Number of changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Keys that were added.
    pub fn added_keys(&self) -> Vec<&str> {
        self.changes
            .iter()
            .filter_map(|c| match c {
                ContextKeyChange::Added(k, _) => Some(k.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Keys that were removed.
    pub fn removed_keys(&self) -> Vec<&str> {
        self.changes
            .iter()
            .filter_map(|c| match c {
                ContextKeyChange::Removed(k, _) => Some(k.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Keys whose values changed.
    pub fn changed_keys(&self) -> Vec<&str> {
        self.changes
            .iter()
            .filter_map(|c| match c {
                ContextKeyChange::Changed(k, _, _) => Some(k.as_str()),
                _ => None,
            })
            .collect()
    }
}

impl fmt::Display for ContextKeyDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.changes.is_empty() {
            return write!(f, "(no changes)");
        }
        for change in &self.changes {
            match change {
                ContextKeyChange::Added(k, v) => writeln!(f, "+ {k} = {v}")?,
                ContextKeyChange::Removed(k, v) => writeln!(f, "- {k} = {v}")?,
                ContextKeyChange::Changed(k, old, new) => {
                    writeln!(f, "~ {k}: {old} -> {new}")?;
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ContextKeyBatch
// ---------------------------------------------------------------------------

/// A batch of context key changes to apply atomically.
///
/// Collects `set` and `remove` operations and applies them in a single
/// write-lock acquisition to avoid intermediate inconsistent states.
#[derive(Debug, Clone, Default)]
pub struct ContextKeyBatch {
    ops: Vec<BatchOp>,
}

#[derive(Debug, Clone)]
enum BatchOp {
    Set(String, ContextKeyValue),
    Remove(String),
}

impl ContextKeyBatch {
    /// Create a new empty batch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a key to be set.
    pub fn set(&mut self, key: impl Into<String>, value: ContextKeyValue) -> &mut Self {
        self.ops.push(BatchOp::Set(key.into(), value));
        self
    }

    /// Queue a key to be removed.
    pub fn remove(&mut self, key: impl Into<String>) -> &mut Self {
        self.ops.push(BatchOp::Remove(key.into()));
        self
    }

    /// Number of queued operations.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Apply all queued operations to a [`ContextKeyService`] atomically
    /// (single write-lock acquisition).
    pub fn apply(&self, service: &ContextKeyService) {
        let mut guard = service.values.write().unwrap();
        for op in &self.ops {
            match op {
                BatchOp::Set(key, value) => {
                    guard.insert(key.clone(), value.clone());
                }
                BatchOp::Remove(key) => {
                    guard.remove(key);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ContextKeyValidator
// ---------------------------------------------------------------------------

/// Result of validating a when-clause expression.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationResult {
    /// Keys referenced in the expression that are not in the known set.
    pub unknown_keys: Vec<String>,
    /// Warnings about potential issues (e.g., comparing a bool key with `>`).
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Whether the expression passed validation with no issues.
    pub fn is_ok(&self) -> bool {
        self.unknown_keys.is_empty() && self.warnings.is_empty()
    }
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_ok() {
            return write!(f, "validation passed");
        }
        if !self.unknown_keys.is_empty() {
            writeln!(f, "unknown keys: {}", self.unknown_keys.join(", "))?;
        }
        for w in &self.warnings {
            writeln!(f, "warning: {w}")?;
        }
        Ok(())
    }
}

/// Validates when-clause expressions against a set of known context keys
/// and their expected types.
#[derive(Debug, Clone)]
pub struct ContextKeyValidator {
    /// Maps known key names to their expected type name ("bool", "string", "number").
    known_keys: HashMap<String, &'static str>,
}

impl ContextKeyValidator {
    /// Create a new validator with no known keys.
    pub fn new() -> Self {
        Self {
            known_keys: HashMap::new(),
        }
    }

    /// Register a known key with its expected type.
    ///
    /// `type_name` should be one of `"bool"`, `"string"`, `"number"`.
    pub fn register(&mut self, key: impl Into<String>, type_name: &'static str) -> &mut Self {
        self.known_keys.insert(key.into(), type_name);
        self
    }

    /// Validate an expression, reporting unknown keys and type-mismatch warnings.
    pub fn validate(&self, expr: &ContextKeyExpr) -> ValidationResult {
        let mut unknown_keys = Vec::new();
        let mut warnings = Vec::new();
        self.validate_inner(expr, &mut unknown_keys, &mut warnings);
        unknown_keys.sort();
        unknown_keys.dedup();
        ValidationResult {
            unknown_keys,
            warnings,
        }
    }

    fn validate_inner(
        &self,
        expr: &ContextKeyExpr,
        unknown: &mut Vec<String>,
        warnings: &mut Vec<String>,
    ) {
        match expr {
            ContextKeyExpr::True | ContextKeyExpr::False => {}
            ContextKeyExpr::Defined(k) => {
                self.check_known(k, unknown);
            }
            ContextKeyExpr::Not(inner) => {
                self.validate_inner(inner, unknown, warnings);
            }
            ContextKeyExpr::Equals(k, _) | ContextKeyExpr::NotEquals(k, _) => {
                self.check_known(k, unknown);
            }
            ContextKeyExpr::Regex(k, _) => {
                self.check_known(k, unknown);
                if let Some(&ty) = self.known_keys.get(k) {
                    if ty != "string" {
                        warnings.push(format!(
                            "regex match on '{k}' which has type '{ty}', expected 'string'"
                        ));
                    }
                }
            }
            ContextKeyExpr::In(k, s) | ContextKeyExpr::NotIn(k, s) => {
                self.check_known(k, unknown);
                self.check_known(s, unknown);
            }
            ContextKeyExpr::Greater(k, _)
            | ContextKeyExpr::GreaterEquals(k, _)
            | ContextKeyExpr::Less(k, _)
            | ContextKeyExpr::LessEquals(k, _) => {
                self.check_known(k, unknown);
                if let Some(&ty) = self.known_keys.get(k) {
                    if ty == "bool" {
                        warnings.push(format!(
                            "numeric comparison on '{k}' which has type 'bool'"
                        ));
                    }
                }
            }
            ContextKeyExpr::And(exprs) | ContextKeyExpr::Or(exprs) => {
                for e in exprs {
                    self.validate_inner(e, unknown, warnings);
                }
            }
        }
    }

    fn check_known(&self, key: &str, unknown: &mut Vec<String>) {
        if !self.known_keys.contains_key(key) {
            unknown.push(key.to_owned());
        }
    }
}

impl Default for ContextKeyValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// ContextKeySerializer — persistence for context key state
// ---------------------------------------------------------------------------

/// Serializes context key state to a portable string representation.
pub struct ContextKeySerializer;

impl ContextKeySerializer {
    /// Serialize a set of context keys to a single string.
    pub fn serialize(keys: &HashMap<String, ContextKeyValue>) -> String {
        let mut entries: Vec<String> = keys
            .iter()
            .map(|(k, v)| format!("{}={}", k, context_key_serialize(v)))
            .collect();
        entries.sort();
        entries.join(";")
    }

    /// Deserialize a serialized string back into key-value pairs.
    pub fn deserialize(s: &str) -> HashMap<String, ContextKeyValue> {
        let mut map = HashMap::new();
        if s.is_empty() {
            return map;
        }
        for entry in s.split(';') {
            if let Some(eq_pos) = entry.find('=') {
                let key = entry[..eq_pos].to_string();
                let val = context_key_deserialize(&entry[eq_pos + 1..]);
                map.insert(key, val);
            }
        }
        map
    }
}

// ---------------------------------------------------------------------------
// ContextKeyDebugView — shows all active keys
// ---------------------------------------------------------------------------

/// Debug view showing all active keys and their values.
pub struct ContextKeyDebugView {
    entries: Vec<(String, ContextKeyValue)>,
}

impl ContextKeyDebugView {
    /// Build a debug view from a context key service.
    pub fn from_service(svc: &ContextKeyService) -> Self {
        let keys = svc.all_keys();
        let mut entries = Vec::new();
        for k in keys {
            if let Some(v) = svc.get_context(&k) {
                entries.push((k, v));
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        Self { entries }
    }

    /// Number of entries in the debug view.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the debug view is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get a reference to all entries.
    pub fn entries(&self) -> &[(String, ContextKeyValue)] {
        &self.entries
    }

    /// Find a key in the debug view.
    pub fn find(&self, key: &str) -> Option<&ContextKeyValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Format as a readable table.
    pub fn format_table(&self) -> String {
        let mut out = String::new();
        for (k, v) in &self.entries {
            out.push_str(&format!("{}: {}\n", k, v));
        }
        out
    }
}

impl fmt::Display for ContextKeyDebugView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContextKeyDebugView({} keys)", self.entries.len())
    }
}

// ---------------------------------------------------------------------------
// ContextKeyHistory — tracks changes to context keys
// ---------------------------------------------------------------------------

/// A record of a single context key change.
#[derive(Debug, Clone)]
pub struct ContextKeyMutation {
    pub key: String,
    pub old_value: Option<ContextKeyValue>,
    pub new_value: Option<ContextKeyValue>,
    pub timestamp: u64,
}

impl fmt::Display for ContextKeyMutation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}: {:?} -> {:?}", self.key, self.timestamp, self.old_value, self.new_value)
    }
}

/// Tracks a history of context key changes.
pub struct ContextKeyHistory {
    changes: Vec<ContextKeyMutation>,
    max_entries: usize,
}

impl ContextKeyHistory {
    pub fn new(max_entries: usize) -> Self {
        Self { changes: Vec::new(), max_entries }
    }

    pub fn record(&mut self, key: impl Into<String>, old: Option<ContextKeyValue>, new: Option<ContextKeyValue>, timestamp: u64) {
        if self.changes.len() >= self.max_entries {
            self.changes.remove(0);
        }
        self.changes.push(ContextKeyMutation { key: key.into(), old_value: old, new_value: new, timestamp });
    }

    pub fn changes(&self) -> &[ContextKeyMutation] { &self.changes }

    pub fn changes_for_key(&self, key: &str) -> Vec<&ContextKeyMutation> {
        self.changes.iter().filter(|c| c.key == key).collect()
    }

    pub fn len(&self) -> usize { self.changes.len() }
    pub fn is_empty(&self) -> bool { self.changes.is_empty() }
    pub fn clear(&mut self) { self.changes.clear(); }

    pub fn changes_since(&self, timestamp: u64) -> Vec<&ContextKeyMutation> {
        self.changes.iter().filter(|c| c.timestamp >= timestamp).collect()
    }

    pub fn last_change(&self) -> Option<&ContextKeyMutation> { self.changes.last() }

    pub fn changed_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.changes.iter().map(|c| c.key.clone()).collect();
        keys.sort();
        keys.dedup();
        keys
    }
}

impl Default for ContextKeyHistory {
    fn default() -> Self { Self::new(1000) }
}

impl fmt::Display for ContextKeyHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContextKeyHistory({} changes)", self.changes.len())
    }
}

// ---------------------------------------------------------------------------
// ContextKeyEventBatch — batch notification of changes
// ---------------------------------------------------------------------------

/// Batch of context key change events for efficient notification.
pub struct ContextKeyEventBatch {
    events: Vec<ContextKeyMutation>,
}

impl ContextKeyEventBatch {
    pub fn new() -> Self { Self { events: Vec::new() } }

    pub fn add(&mut self, key: impl Into<String>, old: Option<ContextKeyValue>, new: Option<ContextKeyValue>, timestamp: u64) {
        self.events.push(ContextKeyMutation { key: key.into(), old_value: old, new_value: new, timestamp });
    }

    pub fn len(&self) -> usize { self.events.len() }
    pub fn is_empty(&self) -> bool { self.events.is_empty() }

    pub fn affected_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.events.iter().map(|e| e.key.as_str()).collect();
        keys.sort();
        keys.dedup();
        keys
    }

    pub fn drain(&mut self) -> Vec<ContextKeyMutation> { std::mem::take(&mut self.events) }
    pub fn events(&self) -> &[ContextKeyMutation] { &self.events }
}

impl Default for ContextKeyEventBatch {
    fn default() -> Self { Self::new() }
}

impl fmt::Display for ContextKeyEventBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContextKeyEventBatch({} events)", self.events.len())
    }
}

// ---------------------------------------------------------------------------
// ContextKeyExprOptimizer - context key expression optimizer
// ---------------------------------------------------------------------------

/// Severity level for context key expression optimizer issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContextKeyExprOptimizerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ContextKeyExprOptimizerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ContextKeyExprOptimizer].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextKeyExprOptimizerEntry {
    pub id: String,
    pub label: String,
    pub severity: ContextKeyExprOptimizerSeverity,
    pub detail: Option<String>,
    pub key_count: usize,
    enabled: bool,
}

impl ContextKeyExprOptimizerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ContextKeyExprOptimizerSeverity::Low,
            detail: None,
            key_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ContextKeyExprOptimizerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_key_count(mut self, val: usize) -> Self {
        self.key_count = val;
        self
    }

    pub fn is_optimized(&self) -> bool {
        self.enabled && self.severity >= ContextKeyExprOptimizerSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.key_count, det)
    }
}

impl fmt::Display for ContextKeyExprOptimizerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ContextKeyExprOptimizerEntry] items.
#[derive(Debug, Clone)]
pub struct ContextKeyExprOptimizer {
    entries: Vec<ContextKeyExprOptimizerEntry>,
    name: String,
    capacity: usize,
}

impl ContextKeyExprOptimizer {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ContextKeyExprOptimizerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ContextKeyExprOptimizerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ContextKeyExprOptimizerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn key_count(&self) -> usize { self.entries.len() }

    pub fn is_optimized(&self) -> bool {
        self.entries.iter().any(|e| e.is_optimized())
    }

    pub fn entries_by_severity(&self, severity: ContextKeyExprOptimizerSeverity) -> Vec<&ContextKeyExprOptimizerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ContextKeyExprOptimizerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ContextKeyExprOptimizerEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&ContextKeyExprOptimizerEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// ContextKeyDebugDump - context key debug dump
// ---------------------------------------------------------------------------

/// Configuration for [ContextKeyDebugDump].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextKeyDebugDumpConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub expr_depth: usize,
}

impl ContextKeyDebugDumpConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, expr_depth: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_expr_depth(mut self, val: usize) -> Self { self.expr_depth = val; self }
}

impl Default for ContextKeyDebugDumpConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ContextKeyDebugDump].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextKeyDebugDumpItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ContextKeyDebugDumpItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn has_keys(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ContextKeyDebugDumpItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ContextKeyDebugDumpItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ContextKeyDebugDump {
    config: ContextKeyDebugDumpConfig,
    items: Vec<ContextKeyDebugDumpItem>,
}

impl ContextKeyDebugDump {
    pub fn new(config: ContextKeyDebugDumpConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ContextKeyDebugDumpItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ContextKeyDebugDumpItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ContextKeyDebugDumpItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn expr_depth(&self) -> usize { self.items.len() }

    pub fn has_keys(&self) -> bool {
        self.items.iter().any(|i| i.has_keys())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ContextKeyDebugDumpItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ContextKeyDebugDumpItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ContextKeyDebugDumpConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ─── CtxKey LRU Cache ───────────────────────────────────────

/// A simple LRU cache for context key eval.
#[derive(Debug)]
pub struct CtxKeyLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> CtxKeyLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for CtxKeyLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CtxKeyLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── CtxKey Builder & Validator ─────────────────────────────

/// Builder for constructing context key configurations.
#[derive(Debug, Clone)]
pub struct CtxKeyBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl CtxKeyBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<CtxKeyCfg, CtxKeyBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(CtxKeyBuildErr { errors }); }
        Ok(CtxKeyCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated context key configuration.
#[derive(Debug, Clone)]
pub struct CtxKeyCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl CtxKeyCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &CtxKeyCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for CtxKeyCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CtxKeyCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct CtxKeyBuildErr { pub errors: Vec<String> }

impl fmt::Display for CtxKeyBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CtxKeyBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for CtxKeyBuildErr {}



// ---------------------------------------------------------------------------
// contextkey – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for context key evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YContextkeyContextKeyOp {
    Equals,
    NotEquals,
    Regex,
    In,
}

impl YContextkeyContextKeyOp {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Equals => 0,
            Self::NotEquals => 1,
            Self::Regex => 2,
            Self::In => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Equals => "Equals",
            Self::NotEquals => "NotEquals",
            Self::Regex => "Regex",
            Self::In => "In",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YContextkeyContextKeyOp] {
        &[
            YContextkeyContextKeyOp::Equals,
            YContextkeyContextKeyOp::NotEquals,
            YContextkeyContextKeyOp::Regex,
            YContextkeyContextKeyOp::In,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YContextkeyContextKeyOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks context key set data.
#[derive(Debug, Clone)]
pub struct YContextkeyContextKeySet {
    pub keys: Vec<(String, String)>,
    pub frozen: bool,
    pub parent_id: Option<String>,
}

impl YContextkeyContextKeySet {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            frozen: false,
            parent_id: None,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YContextkeyContextKeySet({}: {:?})", "keys", self.keys)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_contextkey_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_contextkey_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_contextkey_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_contextkey_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_contextkey_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_contextkey_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_contextkey_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_contextkey_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// contextkey – Extended context key cache helpers
// ---------------------------------------------------------------------------

/// Priority levels for context key cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZContextkeyPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZContextkeyPriority {
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
    pub fn all_asc() -> [ZContextkeyPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZContextkeyPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks context key cache data.
#[derive(Debug, Clone)]
pub struct ZContextkeyContextKeyCache {
    pub cached_values: Vec<(String, bool)>,
    pub hits: u64,
    pub misses: u64,
}

impl ZContextkeyContextKeyCache {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            cached_values: Vec::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.cached_values.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.cached_values.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.cached_values.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZContextkeyContextKeyCache[hits={:?}, misses={:?}]", self.hits, self.misses)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for context key cache.
pub fn z_contextkey_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_contextkey_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_contextkey_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_contextkey_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_contextkey_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_contextkey_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_contextkey_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 23
// ---------------------------------------------------------------------------

/// Generic object pool `Xc23Pool<T>`.
pub struct Xc23Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc23Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc23PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc23Pool<T> {
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
    pub fn stats(&self) -> Xc23PoolStats {
        Xc23PoolStats {
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

impl<T> Default for Xc23Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc23Scheduler`.
pub struct Xc23Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc23Scheduler {
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

impl Default for Xc23Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_23 hash for the given byte slice.
pub fn xc_23_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_23 convention.
pub fn xc_23_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_24 deepening: state machine + event bus ---

/// States for the Xd24 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd24State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd24State {
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
pub struct Xd24Transition {
    pub from: Xd24State,
    pub to: Xd24State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd24StateMachine {
    current: Xd24State,
    history: Vec<Xd24Transition>,
    step_counter: usize,
}

impl Xd24StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd24State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd24State {
        self.current
    }

    pub fn history(&self) -> &[Xd24Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd24State) -> Result<Xd24State, String> {
        let allowed = match (self.current, target) {
            (Xd24State::Idle, Xd24State::Running) => true,
            (Xd24State::Running, Xd24State::Paused) => true,
            (Xd24State::Running, Xd24State::Done) => true,
            (Xd24State::Paused, Xd24State::Running) => true,
            (Xd24State::Paused, Xd24State::Done) => true,
            (Xd24State::Done, Xd24State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_24: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd24Transition {
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
            "Xd24SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd24State> {
        let prefix = "Xd24SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd24State::Idle),
            "Running" => Some(Xd24State::Running),
            "Paused" => Some(Xd24State::Paused),
            "Done" => Some(Xd24State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd24State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd24 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd24Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd24Event {
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

type Xd24HandlerFn = Box<dyn Fn(&Xd24Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd24EventBus {
    handlers: Vec<(usize, Option<String>, Xd24HandlerFn)>,
    next_id: usize,
    published: Vec<Xd24Event>,
}

impl Xd24EventBus {
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
        F: Fn(&Xd24Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd24Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd24Event) {
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

    pub fn published_events(&self) -> &[Xd24Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #22
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf22Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf22TrieNode {
    children: std::collections::HashMap<char, Xf22TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf22Trie {
    root: Xf22TrieNode,
    count: usize,
}

impl Xf22Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf22TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf22TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf22TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf22BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf22BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 22).
pub struct Xh22SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh22SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 64 as u64,
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

/// A compact bit set supporting boolean operations (variant 22).
pub struct Xh22BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh22BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 22).
pub struct Xi22Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi22Deque<T> {
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
pub struct Xi22Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi22Interval {
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

/// A simple interval tree (variant 22).
pub struct Xi22IntervalTree {
    xi_intervals: Vec<Xi22Interval>,
}

impl Xi22IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi22Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi22Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi22Interval) -> Vec<&Xi22Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi22Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi22Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi22Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi22Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi22Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi22Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 22) ---

/// Disjoint set / union-find for crate 22.
pub struct Xj22UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj22UnionFind {
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

const XJ22_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 22.
pub struct Xj22BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj22BTreeNode<K, V>>>,
    len: usize,
}

struct Xj22BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj22BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj22BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ22_BTREE_ORDER - 1
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
        let mid = XJ22_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj22BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj22BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj22BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj22BTreeNode::xj_new_leaf();
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


// --- xk_22 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk22SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk22SegmentTree {
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
pub struct Xk22DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk22DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_22).
#[derive(Debug, Clone)]
pub struct Xl22Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl22Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_22).
#[derive(Debug, Clone)]
pub struct Xl22SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl22SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm22MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm22MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm22Tokenizer {
    text: String,
}

impl Xm22Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 22.
pub struct Xn22Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn22Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 22 -----

#[derive(Debug, Clone)]
struct Xn22AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn22AvlNode<K, V>>>,
    right: Option<Box<Xn22AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 22.
#[derive(Debug, Clone)]
pub struct Xn22AVL<K, V> {
    root: Option<Box<Xn22AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn22AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn22AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn22AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn22AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn22AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn22AvlNode<K, V>>) -> Box<Xn22AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn22AvlNode<K, V>>) -> Box<Xn22AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn22AvlNode<K, V>>) -> Box<Xn22AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn22AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn22AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn22AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn22AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn22AvlNode<K, V>>) -> &Xn22AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn22AvlNode<K, V>>) -> (Box<Xn22AvlNode<K, V>>, Option<Box<Xn22AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn22AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn22AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn22AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn22AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn22AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn22AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn22AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}

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

    // -- Display / From impls ----------------------------------------------

    #[test]
    fn context_key_value_display() {
        assert_eq!(format!("{}", ContextKeyValue::Bool(true)), "true");
        assert_eq!(format!("{}", ContextKeyValue::String("hi".into())), "hi");
        assert_eq!(format!("{}", ContextKeyValue::Number(3.5)), "3.5");
        assert_eq!(format!("{}", ContextKeyValue::Null), "null");
    }

    #[test]
    fn context_key_value_from_impls() {
        let v: ContextKeyValue = true.into();
        assert_eq!(v, ContextKeyValue::Bool(true));

        let v: ContextKeyValue = 42.0_f64.into();
        assert_eq!(v, ContextKeyValue::Number(42.0));

        let v: ContextKeyValue = "hello".into();
        assert_eq!(v, ContextKeyValue::String("hello".into()));

        let v: ContextKeyValue = String::from("world").into();
        assert_eq!(v, ContextKeyValue::String("world".into()));
    }

    // -- ContextKeySnapshot ------------------------------------------------

    #[test]
    fn snapshot_capture_and_keys() {
        let svc = ContextKeyService::new();
        svc.set_context("beta", ContextKeyValue::Number(2.0));
        svc.set_context("alpha", ContextKeyValue::Bool(true));

        let snap = ContextKeySnapshot::capture(&svc);
        assert_eq!(snap.len(), 2);
        assert!(!snap.is_empty());
        assert_eq!(snap.keys(), vec!["alpha", "beta"]);
        assert_eq!(snap.get("alpha"), Some(&ContextKeyValue::Bool(true)));
        assert_eq!(snap.get("missing"), None);
    }

    #[test]
    fn snapshot_display() {
        let snap = ContextKeySnapshot::from_entries(vec![
            ("key".into(), ContextKeyValue::Bool(true)),
        ]);
        let output = format!("{snap}");
        assert!(output.contains("1 keys"));
        assert!(output.contains("key = true"));
    }

    // -- ContextKeyDiff ----------------------------------------------------

    #[test]
    fn diff_added_removed_changed() {
        let old = ContextKeySnapshot::from_entries(vec![
            ("kept".into(), ContextKeyValue::Bool(true)),
            ("changed".into(), ContextKeyValue::Number(1.0)),
            ("removed".into(), ContextKeyValue::String("gone".into())),
        ]);
        let new = ContextKeySnapshot::from_entries(vec![
            ("kept".into(), ContextKeyValue::Bool(true)),
            ("changed".into(), ContextKeyValue::Number(2.0)),
            ("added".into(), ContextKeyValue::Bool(false)),
        ]);

        let diff = old.diff(&new);
        assert_eq!(diff.len(), 3);
        assert!(!diff.is_empty());
        assert_eq!(diff.added_keys(), vec!["added"]);
        assert_eq!(diff.removed_keys(), vec!["removed"]);
        assert_eq!(diff.changed_keys(), vec!["changed"]);
    }

    #[test]
    fn diff_identical_snapshots() {
        let snap = ContextKeySnapshot::from_entries(vec![
            ("a".into(), ContextKeyValue::Bool(true)),
        ]);
        let diff = snap.diff(&snap);
        assert!(diff.is_empty());
        assert_eq!(format!("{diff}"), "(no changes)");
    }

    #[test]
    fn diff_display() {
        let old = ContextKeySnapshot::from_entries(vec![
            ("x".into(), ContextKeyValue::Number(1.0)),
        ]);
        let new = ContextKeySnapshot::from_entries(vec![
            ("x".into(), ContextKeyValue::Number(2.0)),
        ]);
        let diff = old.diff(&new);
        let output = format!("{diff}");
        assert!(output.contains("~ x: 1 -> 2"));
    }

    // -- ContextKeyBatch ---------------------------------------------------

    #[test]
    fn batch_apply_atomic() {
        let svc = ContextKeyService::new();
        svc.set_context("existing", ContextKeyValue::Bool(true));

        let mut batch = ContextKeyBatch::new();
        batch
            .set("new_key", ContextKeyValue::String("hello".into()))
            .set("existing", ContextKeyValue::Bool(false))
            .remove("existing");
        assert_eq!(batch.len(), 3);
        assert!(!batch.is_empty());

        batch.apply(&svc);

        assert_eq!(
            svc.get_context("new_key"),
            Some(ContextKeyValue::String("hello".into()))
        );
        // "existing" was set then removed in order
        assert_eq!(svc.get_context("existing"), None);
    }

    // -- ContextKeyValidator -----------------------------------------------

    #[test]
    fn validator_unknown_keys() {
        let mut validator = ContextKeyValidator::new();
        validator.register("editorFocus", "bool");
        validator.register("editorLangId", "string");

        let expr = ContextKeyExpr::parse("editorFocus && unknownKey == foo").unwrap();
        let result = validator.validate(&expr);
        assert!(!result.is_ok());
        assert!(result.unknown_keys.contains(&"unknownKey".to_string()));
        assert!(!result.unknown_keys.contains(&"editorFocus".to_string()));
    }

    #[test]
    fn validator_type_mismatch_warnings() {
        let mut validator = ContextKeyValidator::new();
        validator.register("isActive", "bool");
        validator.register("name", "string");

        // Numeric comparison on a bool key should warn
        let expr = ContextKeyExpr::parse("isActive > 5").unwrap();
        let result = validator.validate(&expr);
        assert!(result.unknown_keys.is_empty());
        assert!(!result.warnings.is_empty());
        assert!(result.warnings[0].contains("bool"));

        // Regex on a non-string key should warn
        let expr = ContextKeyExpr::parse("isActive =~ /true/").unwrap();
        let result = validator.validate(&expr);
        assert!(result.warnings[0].contains("regex"));
    }

    #[test]
    fn validator_all_known_passes() {
        let mut validator = ContextKeyValidator::new();
        validator.register("editorFocus", "bool");
        validator.register("lang", "string");

        let expr = ContextKeyExpr::parse("editorFocus && lang == rust").unwrap();
        let result = validator.validate(&expr);
        assert!(result.is_ok());
        assert_eq!(format!("{result}"), "validation passed");
    }

    #[test]
    fn validator_in_checks_both_keys() {
        let mut validator = ContextKeyValidator::new();
        validator.register("lang", "string");
        // "supportedLangs" is not registered

        let expr = ContextKeyExpr::parse("lang in supportedLangs").unwrap();
        let result = validator.validate(&expr);
        assert!(result.unknown_keys.contains(&"supportedLangs".to_string()));
        assert!(!result.unknown_keys.contains(&"lang".to_string()));
    }


    #[test]
    fn serializer_roundtrip() {
        let mut keys = HashMap::new();
        keys.insert("a".into(), ContextKeyValue::Bool(true));
        keys.insert("b".into(), ContextKeyValue::String("hello".into()));
        let serialized = ContextKeySerializer::serialize(&keys);
        let deserialized = ContextKeySerializer::deserialize(&serialized);
        assert_eq!(deserialized.get("a"), Some(&ContextKeyValue::Bool(true)));
        assert_eq!(deserialized.get("b"), Some(&ContextKeyValue::String("hello".into())));
    }

    #[test]
    fn serializer_empty() {
        let keys = HashMap::new();
        let s = ContextKeySerializer::serialize(&keys);
        assert!(s.is_empty());
        assert!(ContextKeySerializer::deserialize(&s).is_empty());
    }

    #[test]
    fn debug_view_from_service() {
        let svc = ContextKeyService::new();
        svc.set_context("editor.visible", ContextKeyValue::Bool(true));
        svc.set_context("theme", ContextKeyValue::String("dark".into()));
        let view = ContextKeyDebugView::from_service(&svc);
        assert_eq!(view.len(), 2);
        assert!(view.find("editor.visible").is_some());
        assert!(view.find("missing").is_none());
    }

    #[test]
    fn debug_view_format_table() {
        let svc = ContextKeyService::new();
        svc.set_context("key1", ContextKeyValue::Number(42.0));
        let view = ContextKeyDebugView::from_service(&svc);
        let table = view.format_table();
        assert!(table.contains("key1"));
    }

    #[test]
    fn debug_view_display() {
        let view = ContextKeyDebugView { entries: vec![] };
        assert_eq!(format!("{view}"), "ContextKeyDebugView(0 keys)");
    }

    #[test]
    fn history_record_and_query() {
        let mut history = ContextKeyHistory::new(10);
        history.record("key1", None, Some(ContextKeyValue::Bool(true)), 100);
        history.record("key2", None, Some(ContextKeyValue::Number(1.0)), 200);
        history.record("key1", Some(ContextKeyValue::Bool(true)), Some(ContextKeyValue::Bool(false)), 300);
        assert_eq!(history.len(), 3);
        assert_eq!(history.changes_for_key("key1").len(), 2);
    }

    #[test]
    fn history_changes_since() {
        let mut history = ContextKeyHistory::new(10);
        history.record("a", None, None, 100);
        history.record("b", None, None, 200);
        history.record("c", None, None, 300);
        assert_eq!(history.changes_since(200).len(), 2);
    }

    #[test]
    fn history_max_entries() {
        let mut history = ContextKeyHistory::new(2);
        history.record("a", None, None, 1);
        history.record("b", None, None, 2);
        history.record("c", None, None, 3);
        assert_eq!(history.len(), 2);
        assert_eq!(history.changes()[0].key, "b");
    }

    #[test]
    fn history_changed_keys() {
        let mut history = ContextKeyHistory::new(10);
        history.record("b", None, None, 1);
        history.record("a", None, None, 2);
        history.record("b", None, None, 3);
        assert_eq!(history.changed_keys(), vec!["a", "b"]);
    }

    #[test]
    fn history_last_change() {
        let mut history = ContextKeyHistory::new(10);
        assert!(history.last_change().is_none());
        history.record("x", None, None, 42);
        assert_eq!(history.last_change().unwrap().key, "x");
    }

    #[test]
    fn event_batch_basic() {
        let mut batch = ContextKeyEventBatch::new();
        assert!(batch.is_empty());
        batch.add("key1", None, Some(ContextKeyValue::Bool(true)), 100);
        batch.add("key2", None, None, 200);
        assert_eq!(batch.len(), 2);
        let keys = batch.affected_keys();
        assert!(keys.contains(&"key1"));
    }

    #[test]
    fn event_batch_drain() {
        let mut batch = ContextKeyEventBatch::new();
        batch.add("a", None, None, 1);
        batch.add("b", None, None, 2);
        let drained = batch.drain();
        assert_eq!(drained.len(), 2);
        assert!(batch.is_empty());
    }

    #[test]
    fn event_batch_display() {
        let batch = ContextKeyEventBatch::new();
        assert_eq!(format!("{batch}"), "ContextKeyEventBatch(0 events)");
    }

    #[test]
    fn context_key_change_display() {
        let change = ContextKeyMutation { key: "foo".into(), old_value: None, new_value: Some(ContextKeyValue::Bool(true)), timestamp: 42 };
        let s = format!("{change}");
        assert!(s.contains("foo"));
    }


#[test]
    fn contextkeyexproptimizer_severity_ordering() {
        assert!(ContextKeyExprOptimizerSeverity::Critical > ContextKeyExprOptimizerSeverity::High);
        assert!(ContextKeyExprOptimizerSeverity::High > ContextKeyExprOptimizerSeverity::Medium);
        assert!(ContextKeyExprOptimizerSeverity::Medium > ContextKeyExprOptimizerSeverity::Low);
    }

    #[test]
    fn contextkeyexproptimizer_severity_display() {
        assert_eq!(ContextKeyExprOptimizerSeverity::Low.to_string(), "low");
        assert_eq!(ContextKeyExprOptimizerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn contextkeyexproptimizer_entry_creation() {
        let e = ContextKeyExprOptimizerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ContextKeyExprOptimizerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn contextkeyexproptimizer_entry_builder() {
        let e = ContextKeyExprOptimizerEntry::new("e2", "Entry 2")
            .with_severity(ContextKeyExprOptimizerSeverity::High)
            .with_detail("some detail")
            .with_key_count(42);
        assert_eq!(e.severity, ContextKeyExprOptimizerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.key_count, 42);
    }

    #[test]
    fn contextkeyexproptimizer_entry_enable_disable() {
        let mut e = ContextKeyExprOptimizerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn contextkeyexproptimizer_add_and_count() {
        let mut mgr = ContextKeyExprOptimizer::new("test");
        mgr.add(ContextKeyExprOptimizerEntry::new("a", "A"));
        mgr.add(ContextKeyExprOptimizerEntry::new("b", "B").with_severity(ContextKeyExprOptimizerSeverity::High));
        assert_eq!(mgr.key_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn contextkeyexproptimizer_remove() {
        let mut mgr = ContextKeyExprOptimizer::new("test");
        mgr.add(ContextKeyExprOptimizerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn contextkeyexproptimizer_capacity() {
        let mut mgr = ContextKeyExprOptimizer::new("test").with_capacity(1);
        assert!(mgr.add(ContextKeyExprOptimizerEntry::new("a", "A")));
        assert!(!mgr.add(ContextKeyExprOptimizerEntry::new("b", "B")));
    }

    #[test]
    fn contextkeyexproptimizer_sorted_by_severity() {
        let mut mgr = ContextKeyExprOptimizer::new("test");
        mgr.add(ContextKeyExprOptimizerEntry::new("lo", "Low"));
        mgr.add(ContextKeyExprOptimizerEntry::new("hi", "High").with_severity(ContextKeyExprOptimizerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ContextKeyExprOptimizerSeverity::Critical);
    }

    #[test]
    fn contextkeyexproptimizer_summary() {
        let mgr = ContextKeyExprOptimizer::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn contextkeydebugdump_config_defaults() {
        let cfg = ContextKeyDebugDumpConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn contextkeydebugdump_item_creation() {
        let item = ContextKeyDebugDumpItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn contextkeydebugdump_add_and_get() {
        let mut mgr = ContextKeyDebugDump::new(ContextKeyDebugDumpConfig::new("test"));
        mgr.add(ContextKeyDebugDumpItem::new("k1", "v1"));
        assert_eq!(mgr.expr_depth(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn contextkeydebugdump_remove_item() {
        let mut mgr = ContextKeyDebugDump::new(ContextKeyDebugDumpConfig::new("test"));
        mgr.add(ContextKeyDebugDumpItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn contextkeydebugdump_sorted_by_priority() {
        let mut mgr = ContextKeyDebugDump::new(ContextKeyDebugDumpConfig::new("test"));
        mgr.add(ContextKeyDebugDumpItem::new("lo", "low").with_priority(1));
        mgr.add(ContextKeyDebugDumpItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn contextkeydebugdump_items_with_tag() {
        let mut mgr = ContextKeyDebugDump::new(ContextKeyDebugDumpConfig::new("test"));
        mgr.add(ContextKeyDebugDumpItem::new("a", "1").with_tag("x"));
        mgr.add(ContextKeyDebugDumpItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn contextkeydebugdump_report() {
        let mgr = ContextKeyDebugDump::new(ContextKeyDebugDumpConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn ctxkey_lru_insert_get() {
        let mut c = CtxKeyLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn ctxkey_lru_eviction() {
        let mut c = CtxKeyLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn ctxkey_lru_hit_ratio() {
        let mut c = CtxKeyLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn ctxkey_lru_clear() {
        let mut c = CtxKeyLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn ctxkey_lru_remove() {
        let mut c = CtxKeyLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn ctxkey_lru_peek() {
        let mut c = CtxKeyLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn ctxkey_builder_valid() {
        let cfg = CtxKeyBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn ctxkey_builder_empty_name() {
        let r = CtxKeyBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn ctxkey_builder_bad_priority() {
        assert!(CtxKeyBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn ctxkey_builder_zero_max() {
        assert!(CtxKeyBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn ctxkey_cfg_merge() {
        let mut a = CtxKeyBuilder::new("a").property("x", "1").build().unwrap();
        let b = CtxKeyBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn ctxkey_cfg_display() {
        let cfg = CtxKeyBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    // -- contextkey extended domain tests ----------------------------------------

    #[test]
    fn y_contextkey_enum_index() {
        assert_eq!(YContextkeyContextKeyOp::Equals.index(), 0);
        assert_eq!(YContextkeyContextKeyOp::NotEquals.index(), 1);
        assert_eq!(YContextkeyContextKeyOp::Regex.index(), 2);
        assert_eq!(YContextkeyContextKeyOp::In.index(), 3);
    }

    #[test]
    fn y_contextkey_enum_label() {
        assert_eq!(YContextkeyContextKeyOp::Equals.label(), "Equals");
        assert_eq!(YContextkeyContextKeyOp::NotEquals.label(), "NotEquals");
        assert_eq!(YContextkeyContextKeyOp::Regex.label(), "Regex");
        assert_eq!(YContextkeyContextKeyOp::In.label(), "In");
    }

    #[test]
    fn y_contextkey_enum_all() {
        let all = YContextkeyContextKeyOp::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_contextkey_enum_is_default() {
        assert!(YContextkeyContextKeyOp::Equals.is_default());
        assert!(!YContextkeyContextKeyOp::In.is_default());
    }

    #[test]
    fn y_contextkey_enum_display() {
        assert_eq!(format!("{}", YContextkeyContextKeyOp::Equals), "Equals");
    }

    #[test]
    fn y_contextkey_struct_new() {
        let s = YContextkeyContextKeySet::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_contextkey_struct_clear() {
        let mut s = YContextkeyContextKeySet::new();
        s.keys.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_contextkey_fingerprint_deterministic() {
        let h1 = y_contextkey_fingerprint("hello");
        let h2 = y_contextkey_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_contextkey_fingerprint("a"), y_contextkey_fingerprint("b"));
    }

    #[test]
    fn y_contextkey_truncate_short() {
        assert_eq!(y_contextkey_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_contextkey_truncate_long() {
        let r = y_contextkey_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_contextkey_normalize_key_basic() {
        assert_eq!(y_contextkey_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_contextkey_split_path_basic() {
        let parts = y_contextkey_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_contextkey_count_occurrences_basic() {
        assert_eq!(y_contextkey_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_contextkey_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_contextkey_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_contextkey_in_range_basic() {
        assert!(y_contextkey_in_range(5, 1, 10));
        assert!(y_contextkey_in_range(1, 1, 10));
        assert!(y_contextkey_in_range(10, 1, 10));
        assert!(!y_contextkey_in_range(0, 1, 10));
        assert!(!y_contextkey_in_range(11, 1, 10));
    }

    #[test]
    fn y_contextkey_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_contextkey_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_contextkey_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_contextkey_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- contextkey Z-extended tests -----------------------------------------------

    #[test]
    fn z_contextkey_priority_weight() {
        assert_eq!(ZContextkeyPriority::Idle.weight(), 0);
        assert_eq!(ZContextkeyPriority::Normal.weight(), 2);
        assert_eq!(ZContextkeyPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_contextkey_priority_label() {
        assert_eq!(ZContextkeyPriority::Low.label(), "low");
        assert_eq!(ZContextkeyPriority::High.label(), "high");
    }

    #[test]
    fn z_contextkey_priority_is_elevated() {
        assert!(!ZContextkeyPriority::Normal.is_elevated());
        assert!(ZContextkeyPriority::High.is_elevated());
        assert!(ZContextkeyPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_contextkey_priority_display() {
        assert_eq!(format!("{}", ZContextkeyPriority::Idle), "idle");
    }

    #[test]
    fn z_contextkey_priority_all_asc() {
        let all = ZContextkeyPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZContextkeyPriority::Idle);
        assert_eq!(all[4], ZContextkeyPriority::Realtime);
    }

    #[test]
    fn z_contextkey_struct_new() {
        let s = ZContextkeyContextKeyCache::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_contextkey_struct_toggled_clone() {
        let s = ZContextkeyContextKeyCache::new();
        let t = s.toggled_clone();
        let _ = t.misses;
    }

    #[test]
    fn z_contextkey_rolling_hash_deterministic() {
        let h1 = z_contextkey_rolling_hash(b"test");
        let h2 = z_contextkey_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_contextkey_rolling_hash(b"a"), z_contextkey_rolling_hash(b"b"));
    }

    #[test]
    fn z_contextkey_pad_to_basic() {
        assert_eq!(z_contextkey_pad_to("hi", 5), "hi   ");
        assert_eq!(z_contextkey_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_contextkey_is_identifier_basic() {
        assert!(z_contextkey_is_identifier("foo_bar"));
        assert!(z_contextkey_is_identifier("abc123"));
        assert!(!z_contextkey_is_identifier(""));
        assert!(!z_contextkey_is_identifier("has space"));
    }

    #[test]
    fn z_contextkey_levenshtein_basic() {
        assert_eq!(z_contextkey_levenshtein("", ""), 0);
        assert_eq!(z_contextkey_levenshtein("abc", "abc"), 0);
        assert_eq!(z_contextkey_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_contextkey_unique_words_basic() {
        let w = z_contextkey_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_contextkey_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_contextkey_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_contextkey_common_prefix_basic() {
        assert_eq!(z_contextkey_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_contextkey_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_contextkey_struct_clear() {
        let mut s = ZContextkeyContextKeyCache::new();
        s.cached_values.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_contextkey_rolling_hash_empty() {
        let h = z_contextkey_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 23 ----

    #[test]
    fn xc_23_pool_new_empty() {
        let pool: super::Xc23Pool<i32> = super::Xc23Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_23_pool_release_acquire() {
        let mut pool = super::Xc23Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_23_pool_acquire_empty() {
        let mut pool: super::Xc23Pool<i32> = super::Xc23Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_23_pool_full() {
        let mut pool = super::Xc23Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_23_pool_drain() {
        let mut pool = super::Xc23Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_23_pool_stats() {
        let mut pool = super::Xc23Pool::new(8);
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
    fn xc_23_pool_clear() {
        let mut pool = super::Xc23Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_23_pool_shrink() {
        let mut pool = super::Xc23Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_23_pool_default() {
        let pool: super::Xc23Pool<String> = super::Xc23Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_23_pool_extend() {
        let mut pool = super::Xc23Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_23_pool_retain() {
        let mut pool = super::Xc23Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_23_scheduler_round_robin() {
        let mut sched = super::Xc23Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_23_scheduler_empty() {
        let mut sched = super::Xc23Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_23_scheduler_reset() {
        let mut sched = super::Xc23Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_23_scheduler_add_remove() {
        let mut sched = super::Xc23Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_23_scheduler_targets() {
        let sched = super::Xc23Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_23_hash_empty() {
        assert_eq!(super::xc_23_hash(b""), 5381);
    }

    #[test]
    fn xc_23_hash_data() {
        let h = super::xc_23_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_23_hash(b"hello"), h);
    }

    #[test]
    fn xc_23_reverse_str() {
        assert_eq!(super::xc_23_reverse("abc"), "cba");
        assert_eq!(super::xc_23_reverse(""), "");
    }


    // --- xd_24 deepening tests ---

    #[test]
    fn xd_24_sm_initial_state() {
        let sm = Xd24StateMachine::new();
        assert_eq!(sm.current_state(), Xd24State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_24_sm_valid_idle_to_running() {
        let mut sm = Xd24StateMachine::new();
        assert!(sm.transition(Xd24State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd24State::Running);
    }

    #[test]
    fn xd_24_sm_valid_running_to_paused() {
        let mut sm = Xd24StateMachine::new();
        sm.transition(Xd24State::Running).unwrap();
        assert!(sm.transition(Xd24State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd24State::Paused);
    }

    #[test]
    fn xd_24_sm_valid_running_to_done() {
        let mut sm = Xd24StateMachine::new();
        sm.transition(Xd24State::Running).unwrap();
        assert!(sm.transition(Xd24State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd24State::Done);
    }

    #[test]
    fn xd_24_sm_valid_paused_to_running() {
        let mut sm = Xd24StateMachine::new();
        sm.transition(Xd24State::Running).unwrap();
        sm.transition(Xd24State::Paused).unwrap();
        assert!(sm.transition(Xd24State::Running).is_ok());
    }

    #[test]
    fn xd_24_sm_valid_done_to_idle() {
        let mut sm = Xd24StateMachine::new();
        sm.transition(Xd24State::Running).unwrap();
        sm.transition(Xd24State::Done).unwrap();
        assert!(sm.transition(Xd24State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd24State::Idle);
    }

    #[test]
    fn xd_24_sm_invalid_idle_to_done() {
        let mut sm = Xd24StateMachine::new();
        assert!(sm.transition(Xd24State::Done).is_err());
    }

    #[test]
    fn xd_24_sm_invalid_idle_to_paused() {
        let mut sm = Xd24StateMachine::new();
        assert!(sm.transition(Xd24State::Paused).is_err());
    }

    #[test]
    fn xd_24_sm_history_tracking() {
        let mut sm = Xd24StateMachine::new();
        sm.transition(Xd24State::Running).unwrap();
        sm.transition(Xd24State::Paused).unwrap();
        sm.transition(Xd24State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd24State::Idle);
        assert_eq!(sm.history()[0].to, Xd24State::Running);
        assert_eq!(sm.history()[1].from, Xd24State::Running);
        assert_eq!(sm.history()[2].to, Xd24State::Done);
    }

    #[test]
    fn xd_24_sm_serialize_deserialize() {
        let mut sm = Xd24StateMachine::new();
        sm.transition(Xd24State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd24StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd24State::Running));
    }

    #[test]
    fn xd_24_sm_deserialize_invalid() {
        assert_eq!(Xd24StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_24_sm_reset() {
        let mut sm = Xd24StateMachine::new();
        sm.transition(Xd24State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd24State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_24_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd24EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd24Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_24_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd24EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd24Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd24Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_24_bus_unsubscribe() {
        let mut bus = Xd24EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_24_event_kind_and_payload() {
        let e = Xd24Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd24Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_24_bus_clear_history() {
        let mut bus = Xd24EventBus::new();
        bus.publish(Xd24Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_24_sm_step_counter_increments() {
        let mut sm = Xd24StateMachine::new();
        sm.transition(Xd24State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd24State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #22 --

    #[test]
    fn xf22_trie_insert_search() {
        let mut t = Xf22Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf22_trie_starts_with() {
        let mut t = Xf22Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf22_trie_remove() {
        let mut t = Xf22Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf22_trie_word_count() {
        let mut t = Xf22Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf22_trie_longest_prefix() {
        let mut t = Xf22Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf22_trie_all_words() {
        let mut t = Xf22Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf22_trie_autocomplete() {
        let mut t = Xf22Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf22_trie_empty_search() {
        let t = Xf22Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf22_bloom_add_contains() {
        let mut bf = Xf22BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf22_bloom_probably_absent() {
        let bf = Xf22BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf22_bloom_false_positive_rate() {
        let mut bf = Xf22BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf22_bloom_clear() {
        let mut bf = Xf22BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf22_bloom_union() {
        let mut a = Xf22BloomFilter::xf_new(512, 2);
        let mut b = Xf22BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf22_bloom_intersection_estimate() {
        let mut a = Xf22BloomFilter::xf_new(512, 2);
        let mut b = Xf22BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf22_bloom_union_size_mismatch() {
        let a = Xf22BloomFilter::xf_new(256, 2);
        let b = Xf22BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh22_skip_insert_contains() {
        let mut sl = super::Xh22SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh22_skip_remove() {
        let mut sl = super::Xh22SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh22_skip_len() {
        let mut sl = super::Xh22SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh22_skip_range_query() {
        let mut sl = super::Xh22SkipList::xh_new(4);
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
    fn xh22_skip_floor_ceiling() {
        let mut sl = super::Xh22SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh22_skip_rank() {
        let mut sl = super::Xh22SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh22_skip_empty() {
        let sl = super::Xh22SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh22_skip_duplicates() {
        let mut sl = super::Xh22SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh22_bitset_set_test() {
        let mut bs = super::Xh22BitSet::xh_new(256);
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
    fn xh22_bitset_clear_count() {
        let mut bs = super::Xh22BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh22_bitset_and_or_xor() {
        let mut a = super::Xh22BitSet::xh_new(128);
        let mut b = super::Xh22BitSet::xh_new(128);
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
    fn xh22_bitset_iter_ones() {
        let mut bs = super::Xh22BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh22_bitset_first_last() {
        let mut bs = super::Xh22BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh22_bitset_empty() {
        let bs = super::Xh22BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi22_deque_push_pop_back() {
        let mut dq = super::Xi22Deque::xi_new(4);
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
    fn xi22_deque_push_pop_front() {
        let mut dq = super::Xi22Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi22_deque_mixed_ops() {
        let mut dq = super::Xi22Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi22_deque_get_and_split() {
        let mut dq = super::Xi22Deque::xi_new(8);
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
    fn xi22_deque_rotate_left() {
        let mut dq = super::Xi22Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi22_deque_rotate_right() {
        let mut dq = super::Xi22Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi22_deque_grow() {
        let mut dq = super::Xi22Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi22_deque_empty() {
        let dq = super::Xi22Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi22_interval_tree_insert_query() {
        let mut tree = super::Xi22IntervalTree::xi_new();
        tree.xi_insert(super::Xi22Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi22Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi22Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi22_interval_tree_overlap() {
        let mut tree = super::Xi22IntervalTree::xi_new();
        tree.xi_insert(super::Xi22Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi22Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi22Interval::xi_new(12, 20));
        let q = super::Xi22Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi22_interval_tree_remove() {
        let mut tree = super::Xi22IntervalTree::xi_new();
        tree.xi_insert(super::Xi22Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi22Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi22_interval_tree_gaps() {
        let mut tree = super::Xi22IntervalTree::xi_new();
        tree.xi_insert(super::Xi22Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi22Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi22Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi22Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi22Interval::xi_new(8, 10));
    }

    #[test]
    fn xi22_interval_tree_merge() {
        let mut tree = super::Xi22IntervalTree::xi_new();
        tree.xi_insert(super::Xi22Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi22Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi22Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi22Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi22Interval::xi_new(10, 15));
    }

    #[test]
    fn xi22_interval_tree_all() {
        let mut tree = super::Xi22IntervalTree::xi_new();
        tree.xi_insert(super::Xi22Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi22Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi22_interval_tree_empty() {
        let tree = super::Xi22IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi22_interval_tree_contains_point() {
        let iv = super::Xi22Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 22) ---

    #[test]
    fn xj_22_uf_make_and_find() {
        let mut uf = super::Xj22UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_22_uf_union_connected() {
        let mut uf = super::Xj22UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_22_uf_component_count() {
        let mut uf = super::Xj22UnionFind::xj_new();
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
    fn xj_22_uf_component_size() {
        let mut uf = super::Xj22UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_22_uf_largest_component() {
        let mut uf = super::Xj22UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_22_uf_many_elements() {
        let mut uf = super::Xj22UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_22_uf_separate_components() {
        let mut uf = super::Xj22UnionFind::xj_new();
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
    fn xj_22_uf_path_compression() {
        let mut uf = super::Xj22UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_22_bt_insert_get() {
        let mut bt = super::Xj22BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_22_bt_contains_len() {
        let mut bt = super::Xj22BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_22_bt_replace() {
        let mut bt = super::Xj22BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_22_bt_remove() {
        let mut bt = super::Xj22BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_22_bt_keys_values() {
        let mut bt = super::Xj22BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_22_bt_range() {
        let mut bt = super::Xj22BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_22_bt_min_max() {
        let mut bt = super::Xj22BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_22_bt_many_inserts() {
        let mut bt = super::Xj22BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_22 segment tree tests ---

    #[test]
    fn xk_22_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk22SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_22_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk22SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_22_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk22SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_22_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk22SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_22_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk22SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_22_st_single_element() {
        let data = vec![42];
        let st = super::Xk22SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_22_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk22SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_22_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk22SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_22 disjoint intervals tests ---

    #[test]
    fn xk_22_di_add_and_count() {
        let mut di = super::Xk22DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_22_di_merge_overlap() {
        let mut di = super::Xk22DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_22_di_contains() {
        let mut di = super::Xk22DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_22_di_remove() {
        let mut di = super::Xk22DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_22_di_covered_length() {
        let mut di = super::Xk22DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_22_di_gaps() {
        let mut di = super::Xk22DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_22_di_merge_adjacent() {
        let mut di = super::Xk22DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_22_di_empty() {
        let di = super::Xk22DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_22_rope_new_empty() {
        let rope = super::Xl22Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_22_rope_from_str() {
        let rope = super::Xl22Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_22_rope_insert_at() {
        let mut rope = super::Xl22Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_22_rope_delete_range() {
        let mut rope = super::Xl22Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_22_rope_char_at() {
        let rope = super::Xl22Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_22_rope_split_concat() {
        let rope = super::Xl22Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_22_rope_line_count() {
        let rope = super::Xl22Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_22_rope_line_at() {
        let rope = super::Xl22Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_22_sa_build_and_search() {
        let sa = super::Xl22SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_22_sa_count() {
        let sa = super::Xl22SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_22_sa_longest_repeated() {
        let sa = super::Xl22SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_22_sa_all_positions() {
        let sa = super::Xl22SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_22_sa_len() {
        let sa = super::Xl22SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_22_sa_empty() {
        let sa = super::Xl22SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_22_rope_slice() {
        let rope = super::Xl22Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_22_sa_search_start() {
        let sa = super::Xl22SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_22_sparse_set_get() {
        let mut m = super::Xm22MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_22_sparse_row_col() {
        let mut m = super::Xm22MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_22_sparse_transpose() {
        let mut m = super::Xm22MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_22_sparse_multiply_vec() {
        let mut m = super::Xm22MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_22_sparse_nnz_density() {
        let mut m = super::Xm22MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_22_sparse_clear() {
        let mut m = super::Xm22MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_22_sparse_overwrite_zero() {
        let mut m = super::Xm22MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_22_tokenizer_basic() {
        let t = super::Xm22Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_22_tokenizer_count() {
        let t = super::Xm22Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_22_tokenizer_unique() {
        let t = super::Xm22Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_22_tokenizer_frequency() {
        let t = super::Xm22Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_22_tokenizer_delimiter() {
        let t = super::Xm22Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_22_tokenizer_whitespace() {
        let t = super::Xm22Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_22_tokenizer_empty() {
        let t = super::Xm22Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 22 ----

    #[test]
    fn xn_22_fenwick_prefix_sum() {
        let mut ft = super::Xn22Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_22_fenwick_range_sum() {
        let mut ft = super::Xn22Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_22_fenwick_point_query() {
        let mut ft = super::Xn22Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_22_fenwick_len() {
        let ft = super::Xn22Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_22_fenwick_multiple_updates() {
        let mut ft = super::Xn22Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_22_fenwick_single_element() {
        let mut ft = super::Xn22Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_22_fenwick_find_kth() {
        let mut ft = super::Xn22Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_22_fenwick_negative_delta() {
        let mut ft = super::Xn22Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 22 ----

    #[test]
    fn xn_22_avl_insert_get() {
        let mut m = super::Xn22AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_22_avl_remove() {
        let mut m = super::Xn22AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_22_avl_in_order() {
        let mut m = super::Xn22AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_22_avl_min_max() {
        let mut m = super::Xn22AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_22_avl_floor_ceiling() {
        let mut m = super::Xn22AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_22_avl_height_balanced() {
        let mut m = super::Xn22AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_22_avl_overwrite() {
        let mut m = super::Xn22AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_22_avl_empty() {
        let m: super::Xn22AVL<i32, i32> = super::Xn22AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}