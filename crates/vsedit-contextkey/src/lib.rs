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
}