//! JSON and JSONC parser for vsedit.
//!
//! Equivalent to VS Code's `vs/base/common/json.ts` and `vs/base/common/jsonc.ts`.
//! JSONC (JSON with Comments) supports `//` line comments, `/* */` block
//! comments, and trailing commas — the format used by VS Code's
//! `settings.json`, `keybindings.json`, and `tasks.json`.

use std::fmt;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// A non-fatal parse error with location information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// Comment stripping
// ---------------------------------------------------------------------------

/// Remove comments from JSONC input, replacing comment text with spaces to
/// preserve byte offsets for downstream error reporting.
pub fn strip_comments(input: &str) -> String {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len);
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        if in_string {
            let b = bytes[i];
            if b == b'\\' && i + 1 < len {
                out.push(b as char);
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            out.push(b as char);
            i += 1;
            continue;
        }

        if bytes[i] == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
            continue;
        }

        if bytes[i] == b'/' && i + 1 < len {
            if bytes[i + 1] == b'/' {
                // Line comment: replace with spaces until newline.
                while i < len && bytes[i] != b'\n' {
                    out.push(' ');
                    i += 1;
                }
                continue;
            }
            if bytes[i + 1] == b'*' {
                // Block comment: replace with spaces / preserve newlines.
                out.push(' ');
                out.push(' ');
                i += 2;
                while i < len {
                    if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                        out.push(' ');
                        out.push(' ');
                        i += 2;
                        break;
                    }
                    if bytes[i] == b'\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
                continue;
            }
        }

        out.push(bytes[i] as char);
        i += 1;
    }

    out
}

/// Remove trailing commas from JSON text that has already had comments
/// stripped. Trailing commas appear before `]` or `}` (with optional
/// whitespace in between).
fn strip_trailing_commas(input: &str) -> String {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        let b = bytes[i];

        if in_string {
            if b == b'\\' && i + 1 < len {
                out.push(b);
                out.push(bytes[i + 1]);
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            out.push(b);
            i += 1;
            continue;
        }

        if b == b'"' {
            in_string = true;
            out.push(b);
            i += 1;
            continue;
        }

        if b == b',' {
            // Look ahead past whitespace to see if next meaningful char is ] or }.
            let mut j = i + 1;
            while j < len && (bytes[j] == b' ' || bytes[j] == b'\t' || bytes[j] == b'\n' || bytes[j] == b'\r') {
                j += 1;
            }
            if j < len && (bytes[j] == b']' || bytes[j] == b'}') {
                // Skip the comma (replace with space to keep offsets).
                out.push(b' ');
                i += 1;
                continue;
            }
        }

        out.push(b);
        i += 1;
    }

    // SAFETY: we only ever push ASCII bytes or bytes from the original UTF-8 input.
    unsafe { String::from_utf8_unchecked(out) }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a JSONC string into a [`serde_json::Value`].
///
/// Returns an error if the input cannot be parsed after comment/trailing-comma
/// removal.
pub fn parse_jsonc(input: &str) -> Result<Value, ParseError> {
    let stripped = strip_comments(input);
    let clean = strip_trailing_commas(&stripped);
    serde_json::from_str(&clean).map_err(|e| {
        let (line, column) = serde_error_position(&e);
        ParseError {
            line,
            column,
            message: e.to_string(),
        }
    })
}

/// Parse a JSONC string, collecting errors instead of failing on the first
/// one. Returns `(Some(value), [])` on full success, or
/// `(None, errors)` when parsing fails.
pub fn parse_jsonc_with_errors(input: &str) -> (Option<Value>, Vec<ParseError>) {
    match parse_jsonc(input) {
        Ok(v) => (Some(v), Vec::new()),
        Err(e) => (None, vec![e]),
    }
}

/// Extract line/column from a [`serde_json::Error`].
fn serde_error_position(e: &serde_json::Error) -> (usize, usize) {
    (e.line(), e.column())
}

// ---------------------------------------------------------------------------
// JSON path utilities
// ---------------------------------------------------------------------------

/// Retrieve a nested value by following `path` segments through objects.
///
/// ```
/// use serde_json::json;
/// use vsedit_json::get_value_at_path;
///
/// let v = json!({"a": {"b": 42}});
/// assert_eq!(get_value_at_path(&v, &["a", "b"]), Some(&json!(42)));
/// ```
pub fn get_value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for &key in path {
        current = current.get(key)?;
    }
    Some(current)
}

// ---------------------------------------------------------------------------
// JSON edit operations (formatting-preserving)
// ---------------------------------------------------------------------------

/// Set a nested property in a JSON/JSONC string, preserving existing
/// formatting as much as possible. Comments are **not** preserved across
/// edits (the output is valid JSON).
///
/// `path` is a sequence of object keys. Intermediate objects are created when
/// missing.
pub fn set_property(json: &str, path: &[&str], value: Value) -> String {
    if path.is_empty() {
        return json.to_string();
    }

    let stripped = strip_comments(json);
    let clean = strip_trailing_commas(&stripped);

    let mut root: Value = serde_json::from_str(&clean).unwrap_or(Value::Object(Default::default()));
    set_value_at_path(&mut root, path, value);
    reformat_like(&clean, &root)
}

/// Remove a nested property from a JSON/JSONC string, preserving formatting.
pub fn remove_property(json: &str, path: &[&str]) -> String {
    if path.is_empty() {
        return json.to_string();
    }

    let stripped = strip_comments(json);
    let clean = strip_trailing_commas(&stripped);

    let mut root: Value = match serde_json::from_str(&clean) {
        Ok(v) => v,
        Err(_) => return json.to_string(),
    };

    remove_value_at_path(&mut root, path);
    reformat_like(&clean, &root)
}

/// Mutably walk into `root` along `path`, creating intermediate objects, and
/// set the leaf to `value`.
fn set_value_at_path(root: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        return;
    }

    let mut current = root;
    for &key in &path[..path.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(Default::default());
        }
        current = current
            .as_object_mut()
            .unwrap()
            .entry(key)
            .or_insert_with(|| Value::Object(Default::default()));
    }

    let last = *path.last().unwrap();
    if !current.is_object() {
        *current = Value::Object(Default::default());
    }
    current.as_object_mut().unwrap().insert(last.to_string(), value);
}

/// Walk into `root` along `path` and remove the leaf key.
fn remove_value_at_path(root: &mut Value, path: &[&str]) {
    if path.is_empty() {
        return;
    }
    if path.len() == 1 {
        if let Some(obj) = root.as_object_mut() {
            obj.remove(path[0]);
        }
        return;
    }

    let mut current = root;
    for &key in &path[..path.len() - 1] {
        match current.get_mut(key) {
            Some(v) => current = v,
            None => return,
        }
    }

    let last = *path.last().unwrap();
    if let Some(obj) = current.as_object_mut() {
        obj.remove(last);
    }
}

/// Detect the indentation used in the original source and re-serialize `root`
/// with that indentation style.
fn reformat_like(original: &str, root: &Value) -> String {
    let indent = detect_indent(original);
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    serde::Serialize::serialize(root, &mut ser).expect("Value serialization cannot fail");
    let mut result = String::from_utf8(buf).expect("serde_json always produces valid UTF-8");

    // Preserve trailing newline if the original had one.
    if original.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Detect indentation from the first indented line of the original source.
fn detect_indent(s: &str) -> String {
    for line in s.lines().skip(1) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let leading: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        if !leading.is_empty() {
            return leading;
        }
    }
    // Default to 4 spaces.
    "    ".to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for json operations.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl JsonStats {
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
    pub fn merge(&mut self, other: &JsonStats) {
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

impl Default for JsonStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for JsonStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "JsonStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for json.
#[derive(Debug, Clone)]
pub struct JsonValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl JsonValidator {
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

impl Default for JsonValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// JsonPath – dot-notation access
// ---------------------------------------------------------------------------

/// Provides dot-notation path access to JSON values (e.g., "editor.fontSize").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonPath {
    segments: Vec<String>,
}

impl JsonPath {
    /// Parse a dot-separated path string into a `JsonPath`.
    pub fn parse(path: &str) -> Self {
        let segments = if path.is_empty() {
            Vec::new()
        } else {
            path.split('.').map(String::from).collect()
        };
        Self { segments }
    }

    /// Return the path segments.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Return the depth (number of segments).
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Return the parent path (all segments except the last), or `None` if empty.
    pub fn parent(&self) -> Option<JsonPath> {
        if self.segments.is_empty() {
            return None;
        }
        Some(JsonPath {
            segments: self.segments[..self.segments.len() - 1].to_vec(),
        })
    }

    /// Return a new path with `segment` appended.
    pub fn child(&self, segment: &str) -> JsonPath {
        let mut segments = self.segments.clone();
        segments.push(segment.to_string());
        JsonPath { segments }
    }

    /// Traverse into `value` following the path segments, returning a reference.
    pub fn get<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        let mut current = value;
        for seg in &self.segments {
            match current {
                Value::Object(map) => {
                    current = map.get(seg.as_str())?;
                }
                _ => return None,
            }
        }
        Some(current)
    }

    /// Set a value at this path, creating intermediate objects as needed.
    pub fn set(&self, root: &mut Value, val: Value) {
        if self.segments.is_empty() {
            *root = val;
            return;
        }
        let mut current = root;
        for seg in &self.segments[..self.segments.len() - 1] {
            if !current.is_object() {
                *current = Value::Object(serde_json::Map::new());
            }
            current = current
                .as_object_mut()
                .unwrap()
                .entry(seg.as_str())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
        }
        if !current.is_object() {
            *current = Value::Object(serde_json::Map::new());
        }
        let last = self.segments.last().unwrap();
        current.as_object_mut().unwrap().insert(last.clone(), val);
    }

    /// Remove the value at this path. Returns `true` if a value was removed.
    pub fn remove(&self, root: &mut Value) -> bool {
        if self.segments.is_empty() {
            return false;
        }
        let mut current = root;
        for seg in &self.segments[..self.segments.len() - 1] {
            match current {
                Value::Object(map) => match map.get_mut(seg.as_str()) {
                    Some(v) => current = v,
                    None => return false,
                },
                _ => return false,
            }
        }
        let last = self.segments.last().unwrap();
        match current {
            Value::Object(map) => map.remove(last.as_str()).is_some(),
            _ => false,
        }
    }

    /// Return `true` if the path has no segments.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

impl fmt::Display for JsonPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.segments.join("."))
    }
}

// ---------------------------------------------------------------------------
// JSON minification
// ---------------------------------------------------------------------------

/// Minify a JSON value into a compact single-line string with no extra
/// whitespace. This is the tightest possible JSON representation.
pub fn minify(value: &Value) -> String {
    serde_json::to_string(value).expect("Value serialization cannot fail")
}

/// Minify a JSONC source string: strip comments, trailing commas, parse,
/// then emit compact JSON. Returns an error if the input is not valid JSONC.
pub fn minify_jsonc(input: &str) -> Result<String, ParseError> {
    let value = parse_jsonc(input)?;
    Ok(minify(&value))
}

// ---------------------------------------------------------------------------
// JSON pretty-printing with configurable indentation
// ---------------------------------------------------------------------------

/// Pretty-print a JSON value with the specified indentation string.
///
/// ```
/// use serde_json::json;
/// use vsedit_json::pretty_print;
///
/// let v = json!({"a": 1});
/// let out = pretty_print(&v, "  ");
/// assert!(out.contains("  \"a\": 1"));
/// ```
pub fn pretty_print(value: &Value, indent: &str) -> String {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    serde::Serialize::serialize(value, &mut ser).expect("Value serialization cannot fail");
    String::from_utf8(buf).expect("serde_json always produces valid UTF-8")
}

// ---------------------------------------------------------------------------
// JSON value diffing
// ---------------------------------------------------------------------------

/// Describes a single difference between two JSON value trees.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffEntry {
    /// A key/index exists only in the left tree.
    Removed { path: String, value: Value },
    /// A key/index exists only in the right tree.
    Added { path: String, value: Value },
    /// Both trees have a value at this path but they differ.
    Changed {
        path: String,
        left: Value,
        right: Value,
    },
}

/// Compute the structural diff between two JSON values.
///
/// Returns a list of [`DiffEntry`] items describing every leaf-level
/// difference. Objects are compared recursively; arrays and scalars are
/// compared by equality.
pub fn diff(left: &Value, right: &Value) -> Vec<DiffEntry> {
    let mut entries = Vec::new();
    diff_recursive(left, right, String::new(), &mut entries);
    entries
}

fn diff_recursive(left: &Value, right: &Value, path: String, out: &mut Vec<DiffEntry>) {
    if left == right {
        return;
    }
    match (left, right) {
        (Value::Object(lm), Value::Object(rm)) => {
            for (key, lv) in lm {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };
                match rm.get(key) {
                    Some(rv) => diff_recursive(lv, rv, child_path, out),
                    None => out.push(DiffEntry::Removed {
                        path: child_path,
                        value: lv.clone(),
                    }),
                }
            }
            for (key, rv) in rm {
                if !lm.contains_key(key) {
                    let child_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };
                    out.push(DiffEntry::Added {
                        path: child_path,
                        value: rv.clone(),
                    });
                }
            }
        }
        _ => {
            let p = if path.is_empty() {
                "$".to_string()
            } else {
                path
            };
            out.push(DiffEntry::Changed {
                path: p,
                left: left.clone(),
                right: right.clone(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// JSON value flattening / unflattening
// ---------------------------------------------------------------------------

/// Flatten a nested JSON object into a single-level map with dot-separated
/// keys. Non-object values (arrays, scalars) become leaves.
///
/// ```
/// use serde_json::json;
/// use vsedit_json::json_flatten;
///
/// let v = json!({"a": {"b": 1, "c": 2}});
/// let flat = json_flatten(&v);
/// assert_eq!(flat, json!({"a.b": 1, "a.c": 2}));
/// ```
pub fn json_flatten(value: &Value) -> Value {
    let mut map = serde_json::Map::new();
    flatten_recursive(value, String::new(), &mut map);
    Value::Object(map)
}

fn flatten_recursive(value: &Value, prefix: String, out: &mut serde_json::Map<String, Value>) {
    match value {
        Value::Object(m) => {
            for (key, val) in m {
                let new_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };
                flatten_recursive(val, new_prefix, out);
            }
        }
        other => {
            out.insert(prefix, other.clone());
        }
    }
}

/// Unflatten a single-level JSON object with dot-separated keys back into a
/// nested structure.
///
/// ```
/// use serde_json::json;
/// use vsedit_json::json_unflatten;
///
/// let flat = json!({"a.b": 1, "a.c": 2});
/// let nested = json_unflatten(&flat);
/// assert_eq!(nested, json!({"a": {"b": 1, "c": 2}}));
/// ```
pub fn json_unflatten(value: &Value) -> Value {
    let map = match value.as_object() {
        Some(m) => m,
        None => return value.clone(),
    };
    let mut root = Value::Object(serde_json::Map::new());
    for (dotted_key, val) in map {
        let segments: Vec<&str> = dotted_key.split('.').collect();
        let path = JsonPath {
            segments: segments.iter().map(|s| s.to_string()).collect(),
        };
        path.set(&mut root, val.clone());
    }
    root
}

// ---------------------------------------------------------------------------
// JSON key collection
// ---------------------------------------------------------------------------

/// Collect all unique dot-separated key paths from a JSON value.
/// Useful for editor autocomplete and schema inference.
pub fn collect_keys(value: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    collect_keys_recursive(value, String::new(), &mut keys);
    keys.sort();
    keys
}

fn collect_keys_recursive(value: &Value, prefix: String, out: &mut Vec<String>) {
    if let Value::Object(map) = value {
        for (key, val) in map {
            let full = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };
            out.push(full.clone());
            collect_keys_recursive(val, full, out);
        }
    }
}

// ---------------------------------------------------------------------------
// JSON Patch (simplified RFC 6902)
// ---------------------------------------------------------------------------

/// A single JSON Patch operation (simplified RFC 6902).
#[derive(Debug, Clone, PartialEq)]
pub enum JsonPatchOp {
    Add { path: String, value: Value },
    Remove { path: String },
    Replace { path: String, value: Value },
    Test { path: String, value: Value },
}

/// Apply a sequence of JSON Patch operations to a value.
/// Returns `Ok(())` on success, or an error describing the first failing operation.
pub fn json_patch_apply(target: &mut Value, ops: &[JsonPatchOp]) -> Result<(), String> {
    for (i, op) in ops.iter().enumerate() {
        match op {
            JsonPatchOp::Add { path, value } => {
                let jp = JsonPath::parse(path);
                jp.set(target, value.clone());
            }
            JsonPatchOp::Remove { path } => {
                let jp = JsonPath::parse(path);
                if !jp.remove(target) {
                    return Err(format!(
                        "operation {i}: remove failed – path \"{}\" not found",
                        path
                    ));
                }
            }
            JsonPatchOp::Replace { path, value } => {
                let jp = JsonPath::parse(path);
                if jp.get(target).is_none() {
                    return Err(format!(
                        "operation {i}: replace failed – path \"{}\" not found",
                        path
                    ));
                }
                jp.set(target, value.clone());
            }
            JsonPatchOp::Test { path, value } => {
                let jp = JsonPath::parse(path);
                match jp.get(target) {
                    Some(actual) if actual == value => {}
                    Some(actual) => {
                        return Err(format!(
                            "operation {i}: test failed at \"{}\" – expected {}, got {}",
                            path, value, actual
                        ));
                    }
                    None => {
                        return Err(format!(
                            "operation {i}: test failed – path \"{}\" not found",
                            path
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// json_merge – deep merge
// ---------------------------------------------------------------------------

/// Deep merge two JSON values. The `patch` value is overlaid onto `base`.
/// - Objects are merged recursively.
/// - Arrays and scalars from `patch` replace those in `base`.
/// - Null values in patch remove keys from base.
pub fn json_merge(base: &Value, patch: &Value) -> Value {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            let mut result = base_map.clone();
            for (key, patch_val) in patch_map {
                if patch_val.is_null() {
                    result.remove(key);
                } else if let Some(base_val) = base_map.get(key) {
                    result.insert(key.clone(), json_merge(base_val, patch_val));
                } else {
                    result.insert(key.clone(), patch_val.clone());
                }
            }
            Value::Object(result)
        }
        (_, patch_val) => patch_val.clone(),
    }
}

// ---------------------------------------------------------------------------
// JSON Pointer (RFC 6901)
// ---------------------------------------------------------------------------

/// Resolve a JSON Pointer (RFC 6901) string against a value.
///
/// The pointer must start with `/` or be empty (referring to the root).
/// Segments are separated by `/`, with `~1` unescaped to `/` and `~0` to `~`.
///
/// ```
/// use serde_json::json;
/// use vsedit_json::json_pointer_get;
///
/// let v = json!({"a": {"b": [10, 20, 30]}});
/// assert_eq!(json_pointer_get(&v, "/a/b/1"), Some(&json!(20)));
/// assert_eq!(json_pointer_get(&v, ""), Some(&v));
/// ```
pub fn json_pointer_get<'a>(value: &'a Value, pointer: &str) -> Option<&'a Value> {
    if pointer.is_empty() {
        return Some(value);
    }
    if !pointer.starts_with('/') {
        return None;
    }
    let mut current = value;
    for token in pointer[1..].split('/') {
        let unescaped = json_pointer_unescape(token);
        match current {
            Value::Object(map) => {
                current = map.get(&unescaped)?;
            }
            Value::Array(arr) => {
                let idx: usize = unescaped.parse().ok()?;
                current = arr.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Set a value at the given JSON Pointer path, creating intermediate objects
/// as needed. Array indices are not auto-extended.
pub fn json_pointer_set(root: &mut Value, pointer: &str, val: Value) -> Result<(), String> {
    if pointer.is_empty() {
        *root = val;
        return Ok(());
    }
    if !pointer.starts_with('/') {
        return Err("JSON Pointer must start with '/'".into());
    }
    let tokens: Vec<String> = pointer[1..].split('/').map(json_pointer_unescape).collect();
    let mut current = root;
    for token in &tokens[..tokens.len() - 1] {
        match current {
            Value::Object(map) => {
                current = map
                    .entry(token.as_str())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()));
            }
            Value::Array(arr) => {
                let idx: usize = token
                    .parse()
                    .map_err(|_| format!("invalid array index '{token}'"))?;
                current = arr
                    .get_mut(idx)
                    .ok_or_else(|| format!("array index {idx} out of bounds"))?;
            }
            _ => {
                return Err(format!("cannot traverse into non-container at '{token}'"));
            }
        }
    }
    let last = tokens.last().unwrap();
    match current {
        Value::Object(map) => {
            map.insert(last.clone(), val);
            Ok(())
        }
        Value::Array(arr) => {
            if last == "-" {
                arr.push(val);
                Ok(())
            } else {
                let idx: usize = last
                    .parse()
                    .map_err(|_| format!("invalid array index '{last}'"))?;
                if idx < arr.len() {
                    arr[idx] = val;
                    Ok(())
                } else {
                    Err(format!("array index {idx} out of bounds"))
                }
            }
        }
        _ => Err("cannot set value on a scalar".into()),
    }
}

/// Escape a single token for use in a JSON Pointer string.
pub fn json_pointer_escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Unescape a JSON Pointer token (`~1` → `/`, `~0` → `~`).
fn json_pointer_unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

// ---------------------------------------------------------------------------
// JSON value type helpers
// ---------------------------------------------------------------------------

/// Describes the type of a JSON value as a human-readable string.
pub fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Coerce a JSON value to a boolean using JavaScript-like truthiness rules:
/// - `null` → `false`
/// - `false` → `false`
/// - `0`, `0.0` → `false`
/// - `""` → `false`
/// - everything else → `true`
pub fn json_to_bool(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i != 0
            } else if let Some(f) = n.as_f64() {
                f != 0.0
            } else {
                true
            }
        }
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// Coerce a JSON value to a string.
/// - Strings are returned as-is.
/// - Numbers and booleans are converted to their string representation.
/// - Null returns an empty string.
/// - Arrays and objects are serialized as compact JSON.
pub fn json_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => minify(value),
    }
}

/// Try to coerce a JSON value to an `i64`.
/// - Numbers are converted directly.
/// - Strings are parsed.
/// - Booleans: `true` → 1, `false` → 0.
/// - Everything else returns `None`.
pub fn json_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::Bool(b) => Some(if *b { 1 } else { 0 }),
        Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// JSON value walking / visitor
// ---------------------------------------------------------------------------

/// Callback result for `json_walk` — controls traversal.
pub enum WalkAction {
    /// Continue descending into children.
    Continue,
    /// Skip children of the current node (only meaningful for objects/arrays).
    Skip,
    /// Stop the entire walk immediately.
    Stop,
}

/// Walk every node in a JSON value tree depth-first, calling `visitor` with
/// the dot-separated path and a reference to each node.
///
/// The visitor returns a [`WalkAction`] to control traversal.
pub fn json_walk<F>(value: &Value, mut visitor: F)
where
    F: FnMut(&str, &Value) -> WalkAction,
{
    json_walk_recursive(value, &String::new(), &mut visitor);
}

fn json_walk_recursive<F>(value: &Value, path: &str, visitor: &mut F) -> bool
where
    F: FnMut(&str, &Value) -> WalkAction,
{
    let display_path = if path.is_empty() { "$" } else { path };
    match visitor(display_path, value) {
        WalkAction::Stop => return false,
        WalkAction::Skip => return true,
        WalkAction::Continue => {}
    }
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if !json_walk_recursive(val, &child, visitor) {
                    return false;
                }
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                let child = if path.is_empty() {
                    format!("[{i}]")
                } else {
                    format!("{path}[{i}]")
                };
                if !json_walk_recursive(val, &child, visitor) {
                    return false;
                }
            }
        }
        _ => {}
    }
    true
}

/// Count the total number of leaf nodes (non-object, non-array) in a value.
pub fn json_count_leaves(value: &Value) -> usize {
    let mut count = 0usize;
    json_walk(value, |_, v| {
        if !v.is_object() && !v.is_array() {
            count += 1;
        }
        WalkAction::Continue
    });
    count
}

/// Collect all leaf values as `(path, &Value)` pairs.
pub fn json_collect_leaves(value: &Value) -> Vec<(String, Value)> {
    let mut leaves = Vec::new();
    json_walk(value, |path, v| {
        if !v.is_object() && !v.is_array() {
            leaves.push((path.to_string(), v.clone()));
        }
        WalkAction::Continue
    });
    leaves
}

// ---------------------------------------------------------------------------
// JSON schema-like validation helpers
// ---------------------------------------------------------------------------

/// Describes an expected shape for a JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonSchema {
    /// Any value is accepted.
    Any,
    /// Must be null.
    Null,
    /// Must be a boolean.
    Bool,
    /// Must be a number, optionally within [min, max].
    Number { min: Option<f64>, max: Option<f64> },
    /// Must be a string, optionally with a max length.
    StringType { max_length: Option<usize> },
    /// Must be a string equal to one of the given values.
    Enum(Vec<String>),
    /// Must be an array whose items match the inner schema.
    Array(Box<JsonSchema>),
    /// Must be an object with the specified required/optional fields.
    Object {
        required: Vec<(String, JsonSchema)>,
        optional: Vec<(String, JsonSchema)>,
    },
}

/// A validation error with the path and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Validate a JSON value against a [`JsonSchema`], returning all errors found.
pub fn json_validate(value: &Value, schema: &JsonSchema) -> Vec<SchemaError> {
    let mut errors = Vec::new();
    validate_recursive(value, schema, "$".to_string(), &mut errors);
    errors
}

fn validate_recursive(
    value: &Value,
    schema: &JsonSchema,
    path: String,
    errors: &mut Vec<SchemaError>,
) {
    match schema {
        JsonSchema::Any => {}
        JsonSchema::Null => {
            if !value.is_null() {
                errors.push(SchemaError {
                    path,
                    message: format!("expected null, got {}", json_type_name(value)),
                });
            }
        }
        JsonSchema::Bool => {
            if !value.is_boolean() {
                errors.push(SchemaError {
                    path,
                    message: format!("expected boolean, got {}", json_type_name(value)),
                });
            }
        }
        JsonSchema::Number { min, max } => {
            if let Some(n) = value.as_f64() {
                if let Some(lo) = min {
                    if n < *lo {
                        errors.push(SchemaError {
                            path: path.clone(),
                            message: format!("value {n} is less than minimum {lo}"),
                        });
                    }
                }
                if let Some(hi) = max {
                    if n > *hi {
                        errors.push(SchemaError {
                            path,
                            message: format!("value {n} is greater than maximum {hi}"),
                        });
                    }
                }
            } else {
                errors.push(SchemaError {
                    path,
                    message: format!("expected number, got {}", json_type_name(value)),
                });
            }
        }
        JsonSchema::StringType { max_length } => {
            if let Some(s) = value.as_str() {
                if let Some(max) = max_length {
                    if s.len() > *max {
                        errors.push(SchemaError {
                            path,
                            message: format!(
                                "string length {} exceeds maximum {}",
                                s.len(),
                                max
                            ),
                        });
                    }
                }
            } else {
                errors.push(SchemaError {
                    path,
                    message: format!("expected string, got {}", json_type_name(value)),
                });
            }
        }
        JsonSchema::Enum(variants) => {
            if let Some(s) = value.as_str() {
                if !variants.iter().any(|v| v == s) {
                    errors.push(SchemaError {
                        path,
                        message: format!("value '{}' is not one of {:?}", s, variants),
                    });
                }
            } else {
                errors.push(SchemaError {
                    path,
                    message: format!("expected string for enum, got {}", json_type_name(value)),
                });
            }
        }
        JsonSchema::Array(item_schema) => {
            if let Some(arr) = value.as_array() {
                for (i, item) in arr.iter().enumerate() {
                    validate_recursive(item, item_schema, format!("{path}[{i}]"), errors);
                }
            } else {
                errors.push(SchemaError {
                    path,
                    message: format!("expected array, got {}", json_type_name(value)),
                });
            }
        }
        JsonSchema::Object { required, optional } => {
            if let Some(map) = value.as_object() {
                for (key, field_schema) in required {
                    let child_path = format!("{path}.{key}");
                    match map.get(key) {
                        Some(v) => validate_recursive(v, field_schema, child_path, errors),
                        None => errors.push(SchemaError {
                            path: child_path,
                            message: "required field is missing".into(),
                        }),
                    }
                }
                for (key, field_schema) in optional {
                    if let Some(v) = map.get(key) {
                        let child_path = format!("{path}.{key}");
                        validate_recursive(v, field_schema, child_path, errors);
                    }
                }
            } else {
                errors.push(SchemaError {
                    path,
                    message: format!("expected object, got {}", json_type_name(value)),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JsonDotPathQuery
// ---------------------------------------------------------------------------

/// Query nested JSON values by dot-notation path (e.g., "a.b.c" or "a[0].b").
pub struct JsonDotPathQuery;

impl JsonDotPathQuery {
    /// Get a value at the given dot-notation path.
    pub fn query_value<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
        let mut current = value;
        for segment in Self::parse_segments(path) {
            match segment {
                PathSegment::Key(k) => {
                    current = current.get(&k)?;
                }
                PathSegment::Index(i) => {
                    current = current.get(i)?;
                }
            }
        }
        Some(current)
    }

    /// Set a value at the given path, returning a new Value. Returns None if path is invalid.
    pub fn set_value_at_path(value: &Value, path: &str, new_val: Value) -> Option<Value> {
        let segments = Self::parse_segments(path);
        if segments.is_empty() {
            return Some(new_val);
        }
        let mut root = value.clone();
        Self::set_recursive(&mut root, &segments, new_val)?;
        Some(root)
    }

    fn set_recursive(current: &mut Value, segments: &[PathSegment], new_val: Value) -> Option<()> {
        if segments.len() == 1 {
            match &segments[0] {
                PathSegment::Key(k) => {
                    current.as_object_mut()?.insert(k.clone(), new_val);
                }
                PathSegment::Index(i) => {
                    let arr = current.as_array_mut()?;
                    if *i < arr.len() {
                        arr[*i] = new_val;
                    } else {
                        return None;
                    }
                }
            }
            return Some(());
        }
        let next = match &segments[0] {
            PathSegment::Key(k) => current.get_mut(k.as_str())?,
            PathSegment::Index(i) => current.get_mut(*i)?,
        };
        Self::set_recursive(next, &segments[1..], new_val)
    }

    fn parse_segments(path: &str) -> Vec<PathSegment> {
        let mut segments = Vec::new();
        for part in path.split('.') {
            if part.is_empty() {
                continue;
            }
            if let Some(bracket_pos) = part.find('[') {
                let key = &part[..bracket_pos];
                if !key.is_empty() {
                    segments.push(PathSegment::Key(key.to_string()));
                }
                let idx_str = &part[bracket_pos + 1..part.len() - 1];
                if let Ok(idx) = idx_str.parse::<usize>() {
                    segments.push(PathSegment::Index(idx));
                }
            } else {
                segments.push(PathSegment::Key(part.to_string()));
            }
        }
        segments
    }
}

enum PathSegment {
    Key(String),
    Index(usize),
}

// ---------------------------------------------------------------------------
// JsonPatchBuilder
// ---------------------------------------------------------------------------

/// Builds and applies JSON patches using the existing JsonPatchOp.
pub struct JsonPatchApplier {
    ops: Vec<JsonPatchOp>,
}

impl JsonPatchApplier {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    pub fn add(mut self, path: &str, value: Value) -> Self {
        self.ops.push(JsonPatchOp::Add { path: path.to_string(), value });
        self
    }

    pub fn remove(mut self, path: &str) -> Self {
        self.ops.push(JsonPatchOp::Remove { path: path.to_string() });
        self
    }

    pub fn replace(mut self, path: &str, value: Value) -> Self {
        self.ops.push(JsonPatchOp::Replace { path: path.to_string(), value });
        self
    }

    pub fn op_count(&self) -> usize {
        self.ops.len()
    }

    /// Apply all patches to a value, returning the modified value.
    pub fn apply_all(&self, mut value: Value) -> Value {
        for op in &self.ops {
            match op {
                JsonPatchOp::Add { path, value: v } => {
                    if let Some(result) = JsonDotPathQuery::set_value_at_path(&value, path, v.clone()) {
                        value = result;
                    }
                }
                JsonPatchOp::Remove { path } => {
                    let segments: Vec<&str> = path.rsplitn(2, '.').collect();
                    if segments.len() == 2 {
                        if let Some(parent) = JsonDotPathQuery::query_value(&value, segments[1]) {
                            let mut parent = parent.clone();
                            if let Some(obj) = parent.as_object_mut() {
                                obj.remove(segments[0]);
                                if let Some(result) = JsonDotPathQuery::set_value_at_path(&value, segments[1], parent) {
                                    value = result;
                                }
                            }
                        }
                    } else if let Some(obj) = value.as_object_mut() {
                        obj.remove(path.as_str());
                    }
                }
                JsonPatchOp::Replace { path, value: v } | JsonPatchOp::Test { path, value: v } => {
                    if let Some(result) = JsonDotPathQuery::set_value_at_path(&value, path, v.clone()) {
                        value = result;
                    }
                }
            }
        }
        value
    }
}

// ---------------------------------------------------------------------------
// JsonMinifier
// ---------------------------------------------------------------------------

/// Minifies JSONC by stripping comments and whitespace.
pub struct JsonMinifier;

impl JsonMinifier {
    /// Minify JSONC content to compact JSON.
    pub fn minify(input: &str) -> Option<String> {
        let stripped = strip_comments(input);
        let parsed: Value = serde_json::from_str(&stripped).ok()?;
        Some(serde_json::to_string(&parsed).unwrap_or_default())
    }

    /// Compute savings percentage from minification.
    pub fn savings_percentage(original: &str, minified: &str) -> f64 {
        if original.is_empty() {
            return 0.0;
        }
        let saved = original.len() as f64 - minified.len() as f64;
        (saved / original.len() as f64) * 100.0
    }
}


/// Json processing configuration manager.
#[derive(Debug, Clone)]
pub struct JsonConfig {
    entries: Vec<JsonEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single JSON processing entry.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl JsonEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl JsonConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: JsonEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&JsonEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut JsonEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&JsonEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&JsonEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&JsonEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<JsonEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- strip_comments -----------------------------------------------------

    #[test]
    fn strip_line_comment() {
        let input = r#"{
    "a": 1 // comment
}"#;
        let out = strip_comments(input);
        assert!(!out.contains("//"));
        assert!(!out.contains("comment"));
        assert!(out.contains(r#""a": 1"#));
    }

    #[test]
    fn strip_block_comment() {
        let input = r#"{
    "a": /* block */ 1
}"#;
        let out = strip_comments(input);
        assert!(!out.contains("/*"));
        assert!(!out.contains("*/"));
        assert!(!out.contains("block"));
    }

    #[test]
    fn strip_preserves_string_with_slashes() {
        let input = r#"{"url": "http://example.com"}"#;
        let out = strip_comments(input);
        assert_eq!(out, input);
    }

    #[test]
    fn strip_preserves_string_with_comment_like_content() {
        let input = r#"{"msg": "hello // world"}"#;
        let out = strip_comments(input);
        assert_eq!(out, input);
    }

    #[test]
    fn strip_multiline_block_comment() {
        let input = "{\n/* line1\nline2 */\n\"a\": 1\n}";
        let out = strip_comments(input);
        // Newlines inside block comment should be preserved.
        assert_eq!(out.lines().count(), input.lines().count());
    }

    // -- parse_jsonc --------------------------------------------------------

    #[test]
    fn parse_plain_json() {
        let v = parse_jsonc(r#"{"key": "value"}"#).unwrap();
        assert_eq!(v, json!({"key": "value"}));
    }

    #[test]
    fn parse_with_line_comments() {
        let input = r#"{
    // this is a comment
    "key": 42
}"#;
        let v = parse_jsonc(input).unwrap();
        assert_eq!(v, json!({"key": 42}));
    }

    #[test]
    fn parse_with_block_comments() {
        let input = r#"{
    /* multi
       line */
    "key": true
}"#;
        let v = parse_jsonc(input).unwrap();
        assert_eq!(v, json!({"key": true}));
    }

    #[test]
    fn parse_with_trailing_comma_in_object() {
        let input = r#"{
    "a": 1,
    "b": 2,
}"#;
        let v = parse_jsonc(input).unwrap();
        assert_eq!(v, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn parse_with_trailing_comma_in_array() {
        let input = r#"[1, 2, 3,]"#;
        let v = parse_jsonc(input).unwrap();
        assert_eq!(v, json!([1, 2, 3]));
    }

    #[test]
    fn parse_combined_comments_and_trailing_commas() {
        let input = r#"{
    // comment
    "editor.fontSize": 14,
    "editor.tabSize": 4, // tabs
    /* block */
}"#;
        let v = parse_jsonc(input).unwrap();
        assert_eq!(
            v,
            json!({
                "editor.fontSize": 14,
                "editor.tabSize": 4
            })
        );
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let result = parse_jsonc("{invalid}");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.line > 0);
    }

    // -- parse_jsonc_with_errors --------------------------------------------

    #[test]
    fn parse_with_errors_success() {
        let (v, errs) = parse_jsonc_with_errors(r#"{"a": 1}"#);
        assert!(v.is_some());
        assert!(errs.is_empty());
    }

    #[test]
    fn parse_with_errors_failure() {
        let (v, errs) = parse_jsonc_with_errors("{bad}");
        assert!(v.is_none());
        assert!(!errs.is_empty());
    }

    // -- get_value_at_path --------------------------------------------------

    #[test]
    fn get_path_simple() {
        let v = json!({"a": {"b": {"c": 42}}});
        assert_eq!(get_value_at_path(&v, &["a", "b", "c"]), Some(&json!(42)));
    }

    #[test]
    fn get_path_missing() {
        let v = json!({"a": 1});
        assert_eq!(get_value_at_path(&v, &["a", "b"]), None);
    }

    #[test]
    fn get_path_empty() {
        let v = json!({"a": 1});
        assert_eq!(get_value_at_path(&v, &[]), Some(&v));
    }

    // -- set_property -------------------------------------------------------

    #[test]
    fn set_existing_property() {
        let input = "{\n    \"a\": 1\n}";
        let result = set_property(input, &["a"], json!(2));
        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["a"], json!(2));
    }

    #[test]
    fn set_new_property() {
        let input = "{\n    \"a\": 1\n}";
        let result = set_property(input, &["b"], json!("hello"));
        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["a"], json!(1));
        assert_eq!(v["b"], json!("hello"));
    }

    #[test]
    fn set_nested_property_creates_intermediates() {
        let input = "{}";
        let result = set_property(input, &["a", "b", "c"], json!(true));
        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["a"]["b"]["c"], json!(true));
    }

    #[test]
    fn set_property_preserves_indent() {
        let input = "{\n\t\"a\": 1\n}";
        let result = set_property(input, &["b"], json!(2));
        // Should use tabs, matching the original.
        assert!(result.contains('\t'));
    }

    // -- remove_property ----------------------------------------------------

    #[test]
    fn remove_existing_property() {
        let input = "{\n    \"a\": 1,\n    \"b\": 2\n}";
        let result = remove_property(input, &["a"]);
        let v: Value = serde_json::from_str(&result).unwrap();
        assert!(v.get("a").is_none());
        assert_eq!(v["b"], json!(2));
    }

    #[test]
    fn remove_nested_property() {
        let input = r#"{"a": {"b": 1, "c": 2}}"#;
        let result = remove_property(input, &["a", "b"]);
        let v: Value = serde_json::from_str(&result).unwrap();
        assert!(v["a"].get("b").is_none());
        assert_eq!(v["a"]["c"], json!(2));
    }

    #[test]
    fn remove_missing_property_is_noop() {
        let input = r#"{"a": 1}"#;
        let result = remove_property(input, &["z"]);
        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["a"], json!(1));
    }

    // -- ParseError display -------------------------------------------------

    #[test]
    fn parse_error_display() {
        let e = ParseError {
            line: 3,
            column: 10,
            message: "unexpected token".into(),
        };
        assert_eq!(e.to_string(), "3:10: unexpected token");
    }

    // -- edge cases ---------------------------------------------------------

    #[test]
    fn empty_object() {
        let v = parse_jsonc("{}").unwrap();
        assert_eq!(v, json!({}));
    }

    #[test]
    fn empty_array() {
        let v = parse_jsonc("[]").unwrap();
        assert_eq!(v, json!([]));
    }

    #[test]
    fn nested_trailing_commas() {
        let input = r#"{"a": [1, 2,], "b": {"c": 3,},}"#;
        let v = parse_jsonc(input).unwrap();
        assert_eq!(v, json!({"a": [1, 2], "b": {"c": 3}}));
    }

    #[test]
    fn comment_only_input() {
        let input = "// just a comment\n{}";
        let v = parse_jsonc(input).unwrap();
        assert_eq!(v, json!({}));
    }

    #[test]
    fn string_with_escaped_quote() {
        let input = r#"{"key": "val\"ue"}"#;
        let v = parse_jsonc(input).unwrap();
        assert_eq!(v["key"], json!("val\"ue"));
    }

    #[test]
    fn realistic_settings_json() {
        let input = r#"{
    // Editor settings
    "editor.fontSize": 14,
    "editor.tabSize": 4,
    "editor.wordWrap": "on",

    /* Terminal settings */
    "terminal.integrated.fontSize": 12,

    // File associations
    "files.associations": {
        "*.jsonc": "jsonc",
    },
}"#;
        let v = parse_jsonc(input).unwrap();
        assert_eq!(v["editor.fontSize"], json!(14));
        assert_eq!(v["editor.tabSize"], json!(4));
        assert_eq!(v["terminal.integrated.fontSize"], json!(12));
        assert_eq!(v["files.associations"]["*.jsonc"], json!("jsonc"));
    }

    #[test]
    fn json_stats_new_defaults() {
        let stats = JsonStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn json_stats_record_success() {
        let mut stats = JsonStats::new();
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
    fn json_stats_record_failure() {
        let mut stats = JsonStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn json_stats_reset() {
        let mut stats = JsonStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn json_stats_merge() {
        let mut a = JsonStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = JsonStats::new();
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
    fn json_stats_display() {
        let mut stats = JsonStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn json_stats_default() {
        let stats = JsonStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn json_validator_accepts_valid_name() {
        let v = JsonValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn json_validator_rejects_empty() {
        let v = JsonValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn json_validator_rejects_too_long() {
        let v = JsonValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn json_validator_forbidden_prefix() {
        let v = JsonValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn json_validator_allowed_chars() {
        let v = JsonValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn json_validator_range() {
        let v = JsonValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn json_sanitize_removes_control() {
        let result = JsonValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn json_truncate_short_string() {
        assert_eq!(JsonValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn json_truncate_long_string() {
        let result = JsonValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn json_is_ascii_printable() {
        assert!(JsonValidator::is_ascii_printable("Hello World 123"));
        assert!(!JsonValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- JsonPath -----------------------------------------------------------

    #[test]
    fn json_path_parse_simple() {
        let p = JsonPath::parse("editor.fontSize");
        assert_eq!(p.segments(), &["editor", "fontSize"]);
        assert_eq!(p.depth(), 2);
    }

    #[test]
    fn json_path_parse_single() {
        let p = JsonPath::parse("key");
        assert_eq!(p.segments(), &["key"]);
        assert_eq!(p.depth(), 1);
    }

    #[test]
    fn json_path_parse_empty() {
        let p = JsonPath::parse("");
        assert!(p.is_empty());
        assert_eq!(p.depth(), 0);
    }

    #[test]
    fn json_path_parent_and_child() {
        let p = JsonPath::parse("a.b.c");
        let parent = p.parent().unwrap();
        assert_eq!(parent.to_string(), "a.b");
        let child = parent.child("d");
        assert_eq!(child.to_string(), "a.b.d");
    }

    #[test]
    fn json_path_parent_of_single_is_empty() {
        let p = JsonPath::parse("only");
        let parent = p.parent().unwrap();
        assert!(parent.is_empty());
    }

    #[test]
    fn json_path_parent_of_empty_is_none() {
        let p = JsonPath::parse("");
        assert!(p.parent().is_none());
    }

    #[test]
    fn json_path_get_nested() {
        let val = json!({"a": {"b": {"c": 42}}});
        let p = JsonPath::parse("a.b.c");
        assert_eq!(p.get(&val), Some(&json!(42)));
    }

    #[test]
    fn json_path_get_missing() {
        let val = json!({"a": 1});
        let p = JsonPath::parse("a.b.c");
        assert_eq!(p.get(&val), None);
    }

    #[test]
    fn json_path_set_creates_intermediates() {
        let mut val = json!({});
        let p = JsonPath::parse("a.b.c");
        p.set(&mut val, json!(99));
        assert_eq!(val, json!({"a": {"b": {"c": 99}}}));
    }

    #[test]
    fn json_path_set_overwrites() {
        let mut val = json!({"x": 1});
        let p = JsonPath::parse("x");
        p.set(&mut val, json!(2));
        assert_eq!(val, json!({"x": 2}));
    }

    #[test]
    fn json_path_remove_existing() {
        let mut val = json!({"a": {"b": 1, "c": 2}});
        let p = JsonPath::parse("a.b");
        assert!(p.remove(&mut val));
        assert_eq!(val, json!({"a": {"c": 2}}));
    }

    #[test]
    fn json_path_remove_missing() {
        let mut val = json!({"a": 1});
        let p = JsonPath::parse("z");
        assert!(!p.remove(&mut val));
    }

    #[test]
    fn json_path_to_string_roundtrip() {
        let p = JsonPath::parse("editor.tabSize");
        assert_eq!(p.to_string(), "editor.tabSize");
    }

    // -- json_patch_apply ---------------------------------------------------

    #[test]
    fn patch_add_new_key() {
        let mut v = json!({"a": 1});
        let ops = [JsonPatchOp::Add {
            path: "b".into(),
            value: json!(2),
        }];
        assert!(json_patch_apply(&mut v, &ops).is_ok());
        assert_eq!(v, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn patch_add_nested() {
        let mut v = json!({});
        let ops = [JsonPatchOp::Add {
            path: "x.y".into(),
            value: json!(true),
        }];
        assert!(json_patch_apply(&mut v, &ops).is_ok());
        assert_eq!(v, json!({"x": {"y": true}}));
    }

    #[test]
    fn patch_remove_existing() {
        let mut v = json!({"a": 1, "b": 2});
        let ops = [JsonPatchOp::Remove {
            path: "b".into(),
        }];
        assert!(json_patch_apply(&mut v, &ops).is_ok());
        assert_eq!(v, json!({"a": 1}));
    }

    #[test]
    fn patch_remove_missing_fails() {
        let mut v = json!({"a": 1});
        let ops = [JsonPatchOp::Remove {
            path: "z".into(),
        }];
        assert!(json_patch_apply(&mut v, &ops).is_err());
    }

    #[test]
    fn patch_replace_existing() {
        let mut v = json!({"a": 1});
        let ops = [JsonPatchOp::Replace {
            path: "a".into(),
            value: json!(99),
        }];
        assert!(json_patch_apply(&mut v, &ops).is_ok());
        assert_eq!(v, json!({"a": 99}));
    }

    #[test]
    fn patch_replace_missing_fails() {
        let mut v = json!({"a": 1});
        let ops = [JsonPatchOp::Replace {
            path: "nope".into(),
            value: json!(0),
        }];
        let err = json_patch_apply(&mut v, &ops).unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn patch_test_pass() {
        let mut v = json!({"a": 1});
        let ops = [JsonPatchOp::Test {
            path: "a".into(),
            value: json!(1),
        }];
        assert!(json_patch_apply(&mut v, &ops).is_ok());
    }

    #[test]
    fn patch_test_fail_value() {
        let mut v = json!({"a": 1});
        let ops = [JsonPatchOp::Test {
            path: "a".into(),
            value: json!(2),
        }];
        let err = json_patch_apply(&mut v, &ops).unwrap_err();
        assert!(err.contains("test failed"));
    }

    #[test]
    fn patch_test_fail_missing() {
        let mut v = json!({});
        let ops = [JsonPatchOp::Test {
            path: "x".into(),
            value: json!(1),
        }];
        assert!(json_patch_apply(&mut v, &ops).is_err());
    }

    #[test]
    fn patch_multiple_ops() {
        let mut v = json!({"a": 1});
        let ops = [
            JsonPatchOp::Add { path: "b".into(), value: json!(2) },
            JsonPatchOp::Replace { path: "a".into(), value: json!(10) },
            JsonPatchOp::Test { path: "b".into(), value: json!(2) },
        ];
        assert!(json_patch_apply(&mut v, &ops).is_ok());
        assert_eq!(v, json!({"a": 10, "b": 2}));
    }

    // -- json_merge ---------------------------------------------------------

    #[test]
    fn merge_flat_objects() {
        let base = json!({"a": 1, "b": 2});
        let patch = json!({"b": 20, "c": 30});
        let result = json_merge(&base, &patch);
        assert_eq!(result, json!({"a": 1, "b": 20, "c": 30}));
    }

    #[test]
    fn merge_nested_objects() {
        let base = json!({"editor": {"fontSize": 14, "tabSize": 4}});
        let patch = json!({"editor": {"fontSize": 16}});
        let result = json_merge(&base, &patch);
        assert_eq!(
            result,
            json!({"editor": {"fontSize": 16, "tabSize": 4}})
        );
    }

    #[test]
    fn merge_null_removes_key() {
        let base = json!({"a": 1, "b": 2});
        let patch = json!({"b": null});
        let result = json_merge(&base, &patch);
        assert_eq!(result, json!({"a": 1}));
    }

    #[test]
    fn merge_array_replaces() {
        let base = json!({"list": [1, 2]});
        let patch = json!({"list": [3, 4, 5]});
        let result = json_merge(&base, &patch);
        assert_eq!(result, json!({"list": [3, 4, 5]}));
    }

    #[test]
    fn merge_scalar_replaces() {
        let base = json!({"a": "old"});
        let patch = json!({"a": "new"});
        let result = json_merge(&base, &patch);
        assert_eq!(result, json!({"a": "new"}));
    }

    #[test]
    fn merge_patch_adds_new_keys() {
        let base = json!({"x": 1});
        let patch = json!({"y": 2});
        let result = json_merge(&base, &patch);
        assert_eq!(result, json!({"x": 1, "y": 2}));
    }

    #[test]
    fn merge_non_object_base_replaced() {
        let base = json!(42);
        let patch = json!({"a": 1});
        let result = json_merge(&base, &patch);
        assert_eq!(result, json!({"a": 1}));
    }

    #[test]
    fn merge_deeply_nested() {
        let base = json!({"a": {"b": {"c": 1, "d": 2}}});
        let patch = json!({"a": {"b": {"c": 10, "e": 30}}});
        let result = json_merge(&base, &patch);
        assert_eq!(
            result,
            json!({"a": {"b": {"c": 10, "d": 2, "e": 30}}})
        );
    }

    #[test]
    fn merge_empty_patch_is_noop() {
        let base = json!({"a": 1});
        let patch = json!({});
        let result = json_merge(&base, &patch);
        assert_eq!(result, json!({"a": 1}));
    }

    // -- minify / pretty_print ----------------------------------------------

    #[test]
    fn minify_removes_whitespace() {
        let v = json!({"editor": {"fontSize": 14, "tabSize": 4}});
        let compact = minify(&v);
        assert!(!compact.contains(' '));
        assert!(!compact.contains('\n'));
        assert!(compact.contains("\"editor\""));
        // Round-trip: parse the compact string back
        let parsed: Value = serde_json::from_str(&compact).unwrap();
        assert_eq!(parsed, v);
    }

    #[test]
    fn minify_jsonc_strips_comments_and_compacts() {
        let input = r#"{
            // editor settings
            "a": 1,
            "b": 2,
        }"#;
        let compact = minify_jsonc(input).unwrap();
        assert_eq!(compact, r#"{"a":1,"b":2}"#);
    }

    #[test]
    fn pretty_print_custom_indent() {
        let v = json!({"x": [1, 2]});
        let two_space = pretty_print(&v, "  ");
        let tab = pretty_print(&v, "\t");
        assert!(two_space.contains("  \"x\""));
        assert!(tab.contains("\t\"x\""));
        // Both should parse back to the same value
        let p1: Value = serde_json::from_str(&two_space).unwrap();
        let p2: Value = serde_json::from_str(&tab).unwrap();
        assert_eq!(p1, p2);
    }

    // -- diff ---------------------------------------------------------------

    #[test]
    fn diff_identical_values_yields_empty() {
        let v = json!({"a": {"b": 1}});
        assert!(diff(&v, &v).is_empty());
    }

    #[test]
    fn diff_detects_added_removed_changed() {
        let left = json!({"a": 1, "b": 2, "c": {"d": 3}});
        let right = json!({"a": 1, "b": 20, "e": 5, "c": {"d": 30}});
        let entries = diff(&left, &right);

        // "b" changed
        assert!(entries.iter().any(|e| matches!(e,
            DiffEntry::Changed { path, .. } if path == "b"
        )));
        // "e" added
        assert!(entries.iter().any(|e| matches!(e,
            DiffEntry::Added { path, .. } if path == "e"
        )));
        // "c.d" changed
        assert!(entries.iter().any(|e| matches!(e,
            DiffEntry::Changed { path, .. } if path == "c.d"
        )));
    }

    // -- flatten / unflatten ------------------------------------------------

    #[test]
    fn flatten_and_unflatten_roundtrip() {
        let nested = json!({
            "editor": {
                "fontSize": 14,
                "tabSize": 4
            },
            "terminal": {
                "integrated": {
                    "fontSize": 12
                }
            }
        });
        let flat = json_flatten(&nested);
        assert_eq!(flat["editor.fontSize"], json!(14));
        assert_eq!(flat["terminal.integrated.fontSize"], json!(12));

        let restored = json_unflatten(&flat);
        assert_eq!(restored, nested);
    }

    // -- collect_keys -------------------------------------------------------

    #[test]
    fn collect_keys_returns_all_paths() {
        let v = json!({
            "a": {
                "b": 1,
                "c": {"d": 2}
            },
            "e": 3
        });
        let keys = collect_keys(&v);
        assert!(keys.contains(&"a".to_string()));
        assert!(keys.contains(&"a.b".to_string()));
        assert!(keys.contains(&"a.c".to_string()));
        assert!(keys.contains(&"a.c.d".to_string()));
        assert!(keys.contains(&"e".to_string()));
        assert_eq!(keys.len(), 5);
    }

    // -- json_pointer_get ---------------------------------------------------

    #[test]
    fn pointer_get_root() {
        let v = json!({"a": 1});
        assert_eq!(json_pointer_get(&v, ""), Some(&v));
    }

    #[test]
    fn pointer_get_nested_object() {
        let v = json!({"a": {"b": {"c": 42}}});
        assert_eq!(json_pointer_get(&v, "/a/b/c"), Some(&json!(42)));
    }

    #[test]
    fn pointer_get_array_index() {
        let v = json!({"items": [10, 20, 30]});
        assert_eq!(json_pointer_get(&v, "/items/1"), Some(&json!(20)));
    }

    #[test]
    fn pointer_get_escaped_slash() {
        let v = json!({"a/b": 99});
        assert_eq!(json_pointer_get(&v, "/a~1b"), Some(&json!(99)));
    }

    #[test]
    fn pointer_get_escaped_tilde() {
        let v = json!({"a~b": 77});
        assert_eq!(json_pointer_get(&v, "/a~0b"), Some(&json!(77)));
    }

    #[test]
    fn pointer_get_missing_returns_none() {
        let v = json!({"a": 1});
        assert_eq!(json_pointer_get(&v, "/z"), None);
    }

    #[test]
    fn pointer_get_invalid_no_leading_slash() {
        let v = json!({"a": 1});
        assert_eq!(json_pointer_get(&v, "a"), None);
    }

    #[test]
    fn pointer_set_simple() {
        let mut v = json!({"a": 1});
        json_pointer_set(&mut v, "/b", json!(2)).unwrap();
        assert_eq!(v, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn pointer_set_nested_creates_intermediates() {
        let mut v = json!({});
        json_pointer_set(&mut v, "/x/y/z", json!(true)).unwrap();
        assert_eq!(v, json!({"x": {"y": {"z": true}}}));
    }

    #[test]
    fn pointer_set_array_element() {
        let mut v = json!({"arr": [1, 2, 3]});
        json_pointer_set(&mut v, "/arr/1", json!(99)).unwrap();
        assert_eq!(v["arr"][1], json!(99));
    }

    #[test]
    fn pointer_set_array_append() {
        let mut v = json!({"arr": [1, 2]});
        json_pointer_set(&mut v, "/arr/-", json!(3)).unwrap();
        assert_eq!(v["arr"], json!([1, 2, 3]));
    }

    #[test]
    fn pointer_set_root_replaces() {
        let mut v = json!({"a": 1});
        json_pointer_set(&mut v, "", json!(42)).unwrap();
        assert_eq!(v, json!(42));
    }

    #[test]
    fn pointer_escape_roundtrip() {
        let token = "a/b~c";
        let escaped = json_pointer_escape(token);
        assert_eq!(escaped, "a~1b~0c");
        let unescaped = json_pointer_unescape(&escaped);
        assert_eq!(unescaped, token);
    }

    // -- json_type_name -----------------------------------------------------

    #[test]
    fn type_name_all_variants() {
        assert_eq!(json_type_name(&json!(null)), "null");
        assert_eq!(json_type_name(&json!(true)), "boolean");
        assert_eq!(json_type_name(&json!(42)), "number");
        assert_eq!(json_type_name(&json!("hi")), "string");
        assert_eq!(json_type_name(&json!([1])), "array");
        assert_eq!(json_type_name(&json!({})), "object");
    }

    // -- json_to_bool -------------------------------------------------------

    #[test]
    fn to_bool_falsy_values() {
        assert!(!json_to_bool(&json!(null)));
        assert!(!json_to_bool(&json!(false)));
        assert!(!json_to_bool(&json!(0)));
        assert!(!json_to_bool(&json!("")));
    }

    #[test]
    fn to_bool_truthy_values() {
        assert!(json_to_bool(&json!(true)));
        assert!(json_to_bool(&json!(1)));
        assert!(json_to_bool(&json!(-1)));
        assert!(json_to_bool(&json!("hello")));
        assert!(json_to_bool(&json!([1, 2])));
        assert!(json_to_bool(&json!({"a": 1})));
    }

    // -- json_to_string -----------------------------------------------------

    #[test]
    fn to_string_conversions() {
        assert_eq!(json_to_string(&json!(null)), "");
        assert_eq!(json_to_string(&json!(true)), "true");
        assert_eq!(json_to_string(&json!(42)), "42");
        assert_eq!(json_to_string(&json!("hello")), "hello");
        assert_eq!(json_to_string(&json!([1, 2])), "[1,2]");
    }

    // -- json_to_i64 --------------------------------------------------------

    #[test]
    fn to_i64_conversions() {
        assert_eq!(json_to_i64(&json!(42)), Some(42));
        assert_eq!(json_to_i64(&json!(true)), Some(1));
        assert_eq!(json_to_i64(&json!(false)), Some(0));
        assert_eq!(json_to_i64(&json!("123")), Some(123));
        assert_eq!(json_to_i64(&json!("not a number")), None);
        assert_eq!(json_to_i64(&json!(null)), None);
        assert_eq!(json_to_i64(&json!([1])), None);
    }

    // -- json_walk / json_count_leaves / json_collect_leaves -----------------

    #[test]
    fn walk_visits_all_nodes() {
        let v = json!({"a": 1, "b": {"c": 2}});
        let mut visited = Vec::new();
        json_walk(&v, |path, _val| {
            visited.push(path.to_string());
            WalkAction::Continue
        });
        assert!(visited.contains(&"$".to_string()));
        assert!(visited.contains(&"a".to_string()));
        assert!(visited.contains(&"b".to_string()));
        assert!(visited.contains(&"b.c".to_string()));
    }

    #[test]
    fn walk_skip_prevents_descent() {
        let v = json!({"a": {"deep": 1}, "b": 2});
        let mut visited = Vec::new();
        json_walk(&v, |path, _val| {
            visited.push(path.to_string());
            if path == "a" {
                WalkAction::Skip
            } else {
                WalkAction::Continue
            }
        });
        assert!(visited.contains(&"a".to_string()));
        assert!(!visited.contains(&"a.deep".to_string()));
        assert!(visited.contains(&"b".to_string()));
    }

    #[test]
    fn walk_stop_halts_traversal() {
        let v = json!({"a": 1, "b": 2, "c": 3});
        let mut count = 0usize;
        json_walk(&v, |path, _val| {
            count += 1;
            if path == "a" {
                WalkAction::Stop
            } else {
                WalkAction::Continue
            }
        });
        // Should have visited "$" and then "a" (where it stopped)
        assert_eq!(count, 2);
    }

    #[test]
    fn count_leaves_simple() {
        let v = json!({"a": 1, "b": {"c": 2, "d": 3}, "e": [4, 5]});
        // Leaves: 1, 2, 3, 4, 5 = 5
        assert_eq!(json_count_leaves(&v), 5);
    }

    #[test]
    fn collect_leaves_returns_paths_and_values() {
        let v = json!({"x": 10, "y": {"z": 20}});
        let leaves = json_collect_leaves(&v);
        assert_eq!(leaves.len(), 2);
        assert!(leaves.iter().any(|(p, v)| p == "x" && *v == json!(10)));
        assert!(leaves.iter().any(|(p, v)| p == "y.z" && *v == json!(20)));
    }

    // -- json_validate (schema) ---------------------------------------------

    #[test]
    fn schema_any_accepts_everything() {
        assert!(json_validate(&json!(42), &JsonSchema::Any).is_empty());
        assert!(json_validate(&json!("hi"), &JsonSchema::Any).is_empty());
        assert!(json_validate(&json!(null), &JsonSchema::Any).is_empty());
    }

    #[test]
    fn schema_bool_validation() {
        assert!(json_validate(&json!(true), &JsonSchema::Bool).is_empty());
        let errs = json_validate(&json!("yes"), &JsonSchema::Bool);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("expected boolean"));
    }

    #[test]
    fn schema_number_range_validation() {
        let schema = JsonSchema::Number {
            min: Some(0.0),
            max: Some(100.0),
        };
        assert!(json_validate(&json!(50), &schema).is_empty());
        let errs = json_validate(&json!(-1), &schema);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("less than minimum"));
        let errs = json_validate(&json!(200), &schema);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("greater than maximum"));
    }

    #[test]
    fn schema_string_max_length() {
        let schema = JsonSchema::StringType {
            max_length: Some(5),
        };
        assert!(json_validate(&json!("hi"), &schema).is_empty());
        let errs = json_validate(&json!("toolong"), &schema);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("exceeds maximum"));
    }

    #[test]
    fn schema_enum_validation() {
        let schema = JsonSchema::Enum(vec!["on".into(), "off".into(), "auto".into()]);
        assert!(json_validate(&json!("on"), &schema).is_empty());
        let errs = json_validate(&json!("maybe"), &schema);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("not one of"));
    }

    #[test]
    fn schema_array_validation() {
        let schema = JsonSchema::Array(Box::new(JsonSchema::Number {
            min: None,
            max: None,
        }));
        assert!(json_validate(&json!([1, 2, 3]), &schema).is_empty());
        let errs = json_validate(&json!([1, "two", 3]), &schema);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].path.contains("[1]"));
    }

    #[test]
    fn schema_object_required_fields() {
        let schema = JsonSchema::Object {
            required: vec![
                ("name".into(), JsonSchema::StringType { max_length: None }),
                (
                    "age".into(),
                    JsonSchema::Number {
                        min: Some(0.0),
                        max: None,
                    },
                ),
            ],
            optional: vec![("email".into(), JsonSchema::StringType { max_length: None })],
        };
        let valid = json!({"name": "Alice", "age": 30});
        assert!(json_validate(&valid, &schema).is_empty());

        let missing_name = json!({"age": 30});
        let errs = json_validate(&missing_name, &schema);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("required field"));

        let bad_age = json!({"name": "Bob", "age": -5});
        let errs = json_validate(&bad_age, &schema);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("less than minimum"));
    }

    #[test]
    fn schema_error_display() {
        let e = SchemaError {
            path: "$.name".into(),
            message: "required field is missing".into(),
        };
        assert_eq!(e.to_string(), "$.name: required field is missing");
    }

    // -- json_walk with arrays ----------------------------------------------

    #[test]
    fn walk_array_paths() {
        let v = json!({"items": [10, 20]});
        let mut paths = Vec::new();
        json_walk(&v, |path, _| {
            paths.push(path.to_string());
            WalkAction::Continue
        });
        assert!(paths.contains(&"items[0]".to_string()));
        assert!(paths.contains(&"items[1]".to_string()));
    }

    // -- JsonDotPathQuery tests --

    #[test]
    fn path_query_simple() {
        let v = json!({"a": {"b": 42}});
        assert_eq!(JsonDotPathQuery::query_value(&v, "a.b"), Some(&json!(42)));
    }

    #[test]
    fn path_query_array_index() {
        let v = json!({"items": [10, 20, 30]});
        assert_eq!(JsonDotPathQuery::query_value(&v, "items[1]"), Some(&json!(20)));
    }

    #[test]
    fn path_query_missing() {
        let v = json!({"a": 1});
        assert!(JsonDotPathQuery::query_value(&v, "b.c").is_none());
    }

    #[test]
    fn path_set_value() {
        let v = json!({"a": {"b": 1}});
        let result = JsonDotPathQuery::set_value_at_path(&v, "a.b", json!(99)).unwrap();
        assert_eq!(result["a"]["b"], json!(99));
    }

    // -- JsonPatchApplier tests --

    #[test]
    fn patch_add() {
        let v = json!({"a": 1});
        let result = JsonPatchApplier::new().add("a", json!(2)).apply_all(v);
        assert_eq!(result["a"], json!(2));
    }

    #[test]
    fn patch_replace() {
        let v = json!({"x": "old"});
        let result = JsonPatchApplier::new().replace("x", json!("new")).apply_all(v);
        assert_eq!(result["x"], json!("new"));
    }

    #[test]
    fn patch_remove() {
        let v = json!({"a": 1, "b": 2});
        let result = JsonPatchApplier::new().remove("b").apply_all(v);
        assert!(result.get("b").is_none());
    }

    #[test]
    fn patch_op_count() {
        let p = JsonPatchApplier::new().add("a", json!(1)).remove("b");
        assert_eq!(p.op_count(), 2);
    }

    // -- JsonMinifier tests --

    #[test]
    fn minifier_basic() {
        let input = "{\n  \"a\": 1,\n  \"b\": 2\n}";
        let minified = JsonMinifier::minify(input).unwrap();
        assert!(!minified.contains('\n'));
        assert!(minified.contains("\"a\""));
    }

    #[test]
    fn minifier_strips_comments() {
        let input = "{\n  \"a\": 1 // comment\n}";
        let minified = JsonMinifier::minify(input).unwrap();
        assert!(!minified.contains("comment"));
    }

    #[test]
    fn minifier_savings() {
        let original = "{\n  \"a\": 1\n}";
        let minified = "{\"a\":1}";
        let pct = JsonMinifier::savings_percentage(original, minified);
        assert!(pct > 0.0);
    }

    #[test]
    fn minifier_savings_empty() {
        assert!((JsonMinifier::savings_percentage("", "")).abs() < 0.01);
    }


    #[test]
    fn json_entry_creation() {
        let e = JsonEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn json_entry_with_priority() {
        let e = JsonEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn json_entry_metadata() {
        let e = JsonEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn json_entry_remove_meta() {
        let mut e = JsonEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn json_entry_activate_deactivate() {
        let mut e = JsonEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn json_config_add_sorted() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("lo", "Lo").with_priority(1));
        c.add(JsonEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn json_config_capacity() {
        let mut c = JsonConfig::new(1);
        assert!(c.add(JsonEntry::new("a", "A")));
        assert!(!c.add(JsonEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn json_config_remove() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn json_config_get() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn json_config_active_entries() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("a", "A"));
        c.add(JsonEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn json_config_enable_disable() {
        let mut c = JsonConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn json_config_clear() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn json_config_find_by_label() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn json_config_top_n() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("a", "A").with_priority(1));
        c.add(JsonEntry::new("b", "B").with_priority(2));
        c.add(JsonEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn json_config_deactivate_activate_all() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("a", "A"));
        c.add(JsonEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn json_config_highest_priority() {
        let mut c = JsonConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(JsonEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn json_config_contains() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn json_config_labels() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("a", "Alpha"));
        c.add(JsonEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn json_config_drain_inactive() {
        let mut c = JsonConfig::new(10);
        c.add(JsonEntry::new("a", "A"));
        c.add(JsonEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }

}
