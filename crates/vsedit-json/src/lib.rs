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
// xa_ extended helpers for json
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaJsonRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaJsonRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaJsonCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaJsonCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaJsonCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 100
// ---------------------------------------------------------------------------

/// Generic object pool `Xc100Pool<T>`.
pub struct Xc100Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc100Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc100PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc100Pool<T> {
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
    pub fn stats(&self) -> Xc100PoolStats {
        Xc100PoolStats {
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

impl<T> Default for Xc100Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc100Scheduler`.
pub struct Xc100Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc100Scheduler {
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

impl Default for Xc100Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_100 hash for the given byte slice.
pub fn xc_100_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_100 convention.
pub fn xc_100_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_68 deepening: state machine + event bus ---

/// States for the Xd68 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd68State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd68State {
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
pub struct Xd68Transition {
    pub from: Xd68State,
    pub to: Xd68State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd68StateMachine {
    current: Xd68State,
    history: Vec<Xd68Transition>,
    step_counter: usize,
}

impl Xd68StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd68State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd68State {
        self.current
    }

    pub fn history(&self) -> &[Xd68Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd68State) -> Result<Xd68State, String> {
        let allowed = match (self.current, target) {
            (Xd68State::Idle, Xd68State::Running) => true,
            (Xd68State::Running, Xd68State::Paused) => true,
            (Xd68State::Running, Xd68State::Done) => true,
            (Xd68State::Paused, Xd68State::Running) => true,
            (Xd68State::Paused, Xd68State::Done) => true,
            (Xd68State::Done, Xd68State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_68: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd68Transition {
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
            "Xd68SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd68State> {
        let prefix = "Xd68SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd68State::Idle),
            "Running" => Some(Xd68State::Running),
            "Paused" => Some(Xd68State::Paused),
            "Done" => Some(Xd68State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd68State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd68 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd68Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd68Event {
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

type Xd68HandlerFn = Box<dyn Fn(&Xd68Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd68EventBus {
    handlers: Vec<(usize, Option<String>, Xd68HandlerFn)>,
    next_id: usize,
    published: Vec<Xd68Event>,
}

impl Xd68EventBus {
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
        F: Fn(&Xd68Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd68Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd68Event) {
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

    pub fn published_events(&self) -> &[Xd68Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #77
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf77Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf77TrieNode {
    children: std::collections::HashMap<char, Xf77TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf77Trie {
    root: Xf77TrieNode,
    count: usize,
}

impl Xf77Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf77TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf77TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf77TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf77BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf77BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 99).
pub struct Xh99SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh99SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 141 as u64,
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

/// A compact bit set supporting boolean operations (variant 99).
pub struct Xh99BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh99BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 99).
pub struct Xi99Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi99Deque<T> {
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
pub struct Xi99Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi99Interval {
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

/// A simple interval tree (variant 99).
pub struct Xi99IntervalTree {
    xi_intervals: Vec<Xi99Interval>,
}

impl Xi99IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi99Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi99Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi99Interval) -> Vec<&Xi99Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi99Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi99Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi99Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi99Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi99Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi99Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 100) ---

/// Disjoint set / union-find for crate 100.
pub struct Xj100UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj100UnionFind {
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

const XJ100_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 100.
pub struct Xj100BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj100BTreeNode<K, V>>>,
    len: usize,
}

struct Xj100BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj100BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj100BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ100_BTREE_ORDER - 1
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
        let mid = XJ100_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj100BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj100BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj100BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj100BTreeNode::xj_new_leaf();
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


// --- xk_99 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk99SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk99SegmentTree {
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
pub struct Xk99DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk99DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_100).
#[derive(Debug, Clone)]
pub struct Xl100Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl100Rope {
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

/// Suffix array for efficient string searching (xl_100).
#[derive(Debug, Clone)]
pub struct Xl100SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl100SuffixArray {
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
pub struct Xm100MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm100MatrixSparse {
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
pub struct Xm100Tokenizer {
    text: String,
}

impl Xm100Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 99.
pub struct Xn99Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn99Fenwick {
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

// ----- AVL tree map — crate 99 -----

#[derive(Debug, Clone)]
struct Xn99AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn99AvlNode<K, V>>>,
    right: Option<Box<Xn99AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 99.
#[derive(Debug, Clone)]
pub struct Xn99AVL<K, V> {
    root: Option<Box<Xn99AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn99AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn99AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn99AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn99AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn99AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn99AvlNode<K, V>>) -> Box<Xn99AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn99AvlNode<K, V>>) -> Box<Xn99AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn99AvlNode<K, V>>) -> Box<Xn99AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn99AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn99AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn99AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn99AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn99AvlNode<K, V>>) -> &Xn99AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn99AvlNode<K, V>>) -> (Box<Xn99AvlNode<K, V>>, Option<Box<Xn99AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn99AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn99AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn99AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn99AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn99AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn99AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn99AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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


    // xa_ extended tests for json
    #[test]
    fn xa_json_ring_new() {
        let rb = super::XaJsonRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_json_ring_push_len() {
        let mut rb = super::XaJsonRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_json_ring_wrap() {
        let mut rb = super::XaJsonRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_json_ring_mean_empty() {
        let rb = super::XaJsonRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_json_ring_mean_values() {
        let mut rb = super::XaJsonRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_json_ring_min_max() {
        let mut rb = super::XaJsonRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_json_ring_iter() {
        let mut rb = super::XaJsonRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_json_counter_new() {
        let c = super::XaJsonCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_json_counter_inc() {
        let mut c = super::XaJsonCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_json_counter_inc_by() {
        let mut c = super::XaJsonCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_json_counter_reset() {
        let mut c = super::XaJsonCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_json_counter_clear() {
        let mut c = super::XaJsonCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_json_counter_default() {
        let c = super::XaJsonCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 100 ----

    #[test]
    fn xc_100_pool_new_empty() {
        let pool: super::Xc100Pool<i32> = super::Xc100Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_100_pool_release_acquire() {
        let mut pool = super::Xc100Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_100_pool_acquire_empty() {
        let mut pool: super::Xc100Pool<i32> = super::Xc100Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_100_pool_full() {
        let mut pool = super::Xc100Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_100_pool_drain() {
        let mut pool = super::Xc100Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_100_pool_stats() {
        let mut pool = super::Xc100Pool::new(8);
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
    fn xc_100_pool_clear() {
        let mut pool = super::Xc100Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_100_pool_shrink() {
        let mut pool = super::Xc100Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_100_pool_default() {
        let pool: super::Xc100Pool<String> = super::Xc100Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_100_pool_extend() {
        let mut pool = super::Xc100Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_100_pool_retain() {
        let mut pool = super::Xc100Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_100_scheduler_round_robin() {
        let mut sched = super::Xc100Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_100_scheduler_empty() {
        let mut sched = super::Xc100Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_100_scheduler_reset() {
        let mut sched = super::Xc100Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_100_scheduler_add_remove() {
        let mut sched = super::Xc100Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_100_scheduler_targets() {
        let sched = super::Xc100Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_100_hash_empty() {
        assert_eq!(super::xc_100_hash(b""), 5381);
    }

    #[test]
    fn xc_100_hash_data() {
        let h = super::xc_100_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_100_hash(b"hello"), h);
    }

    #[test]
    fn xc_100_reverse_str() {
        assert_eq!(super::xc_100_reverse("abc"), "cba");
        assert_eq!(super::xc_100_reverse(""), "");
    }


    // --- xd_68 deepening tests ---

    #[test]
    fn xd_68_sm_initial_state() {
        let sm = Xd68StateMachine::new();
        assert_eq!(sm.current_state(), Xd68State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_68_sm_valid_idle_to_running() {
        let mut sm = Xd68StateMachine::new();
        assert!(sm.transition(Xd68State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd68State::Running);
    }

    #[test]
    fn xd_68_sm_valid_running_to_paused() {
        let mut sm = Xd68StateMachine::new();
        sm.transition(Xd68State::Running).unwrap();
        assert!(sm.transition(Xd68State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd68State::Paused);
    }

    #[test]
    fn xd_68_sm_valid_running_to_done() {
        let mut sm = Xd68StateMachine::new();
        sm.transition(Xd68State::Running).unwrap();
        assert!(sm.transition(Xd68State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd68State::Done);
    }

    #[test]
    fn xd_68_sm_valid_paused_to_running() {
        let mut sm = Xd68StateMachine::new();
        sm.transition(Xd68State::Running).unwrap();
        sm.transition(Xd68State::Paused).unwrap();
        assert!(sm.transition(Xd68State::Running).is_ok());
    }

    #[test]
    fn xd_68_sm_valid_done_to_idle() {
        let mut sm = Xd68StateMachine::new();
        sm.transition(Xd68State::Running).unwrap();
        sm.transition(Xd68State::Done).unwrap();
        assert!(sm.transition(Xd68State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd68State::Idle);
    }

    #[test]
    fn xd_68_sm_invalid_idle_to_done() {
        let mut sm = Xd68StateMachine::new();
        assert!(sm.transition(Xd68State::Done).is_err());
    }

    #[test]
    fn xd_68_sm_invalid_idle_to_paused() {
        let mut sm = Xd68StateMachine::new();
        assert!(sm.transition(Xd68State::Paused).is_err());
    }

    #[test]
    fn xd_68_sm_history_tracking() {
        let mut sm = Xd68StateMachine::new();
        sm.transition(Xd68State::Running).unwrap();
        sm.transition(Xd68State::Paused).unwrap();
        sm.transition(Xd68State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd68State::Idle);
        assert_eq!(sm.history()[0].to, Xd68State::Running);
        assert_eq!(sm.history()[1].from, Xd68State::Running);
        assert_eq!(sm.history()[2].to, Xd68State::Done);
    }

    #[test]
    fn xd_68_sm_serialize_deserialize() {
        let mut sm = Xd68StateMachine::new();
        sm.transition(Xd68State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd68StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd68State::Running));
    }

    #[test]
    fn xd_68_sm_deserialize_invalid() {
        assert_eq!(Xd68StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_68_sm_reset() {
        let mut sm = Xd68StateMachine::new();
        sm.transition(Xd68State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd68State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_68_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd68EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd68Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_68_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd68EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd68Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd68Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_68_bus_unsubscribe() {
        let mut bus = Xd68EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_68_event_kind_and_payload() {
        let e = Xd68Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd68Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_68_bus_clear_history() {
        let mut bus = Xd68EventBus::new();
        bus.publish(Xd68Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_68_sm_step_counter_increments() {
        let mut sm = Xd68StateMachine::new();
        sm.transition(Xd68State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd68State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #77 --

    #[test]
    fn xf77_trie_insert_search() {
        let mut t = Xf77Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf77_trie_starts_with() {
        let mut t = Xf77Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf77_trie_remove() {
        let mut t = Xf77Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf77_trie_word_count() {
        let mut t = Xf77Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf77_trie_longest_prefix() {
        let mut t = Xf77Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf77_trie_all_words() {
        let mut t = Xf77Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf77_trie_autocomplete() {
        let mut t = Xf77Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf77_trie_empty_search() {
        let t = Xf77Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf77_bloom_add_contains() {
        let mut bf = Xf77BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf77_bloom_probably_absent() {
        let bf = Xf77BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf77_bloom_false_positive_rate() {
        let mut bf = Xf77BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf77_bloom_clear() {
        let mut bf = Xf77BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf77_bloom_union() {
        let mut a = Xf77BloomFilter::xf_new(512, 2);
        let mut b = Xf77BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf77_bloom_intersection_estimate() {
        let mut a = Xf77BloomFilter::xf_new(512, 2);
        let mut b = Xf77BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf77_bloom_union_size_mismatch() {
        let a = Xf77BloomFilter::xf_new(256, 2);
        let b = Xf77BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh99_skip_insert_contains() {
        let mut sl = super::Xh99SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh99_skip_remove() {
        let mut sl = super::Xh99SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh99_skip_len() {
        let mut sl = super::Xh99SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh99_skip_range_query() {
        let mut sl = super::Xh99SkipList::xh_new(4);
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
    fn xh99_skip_floor_ceiling() {
        let mut sl = super::Xh99SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh99_skip_rank() {
        let mut sl = super::Xh99SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh99_skip_empty() {
        let sl = super::Xh99SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh99_skip_duplicates() {
        let mut sl = super::Xh99SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh99_bitset_set_test() {
        let mut bs = super::Xh99BitSet::xh_new(256);
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
    fn xh99_bitset_clear_count() {
        let mut bs = super::Xh99BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh99_bitset_and_or_xor() {
        let mut a = super::Xh99BitSet::xh_new(128);
        let mut b = super::Xh99BitSet::xh_new(128);
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
    fn xh99_bitset_iter_ones() {
        let mut bs = super::Xh99BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh99_bitset_first_last() {
        let mut bs = super::Xh99BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh99_bitset_empty() {
        let bs = super::Xh99BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi99_deque_push_pop_back() {
        let mut dq = super::Xi99Deque::xi_new(4);
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
    fn xi99_deque_push_pop_front() {
        let mut dq = super::Xi99Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi99_deque_mixed_ops() {
        let mut dq = super::Xi99Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi99_deque_get_and_split() {
        let mut dq = super::Xi99Deque::xi_new(8);
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
    fn xi99_deque_rotate_left() {
        let mut dq = super::Xi99Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi99_deque_rotate_right() {
        let mut dq = super::Xi99Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi99_deque_grow() {
        let mut dq = super::Xi99Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi99_deque_empty() {
        let dq = super::Xi99Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi99_interval_tree_insert_query() {
        let mut tree = super::Xi99IntervalTree::xi_new();
        tree.xi_insert(super::Xi99Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi99Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi99Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi99_interval_tree_overlap() {
        let mut tree = super::Xi99IntervalTree::xi_new();
        tree.xi_insert(super::Xi99Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi99Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi99Interval::xi_new(12, 20));
        let q = super::Xi99Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi99_interval_tree_remove() {
        let mut tree = super::Xi99IntervalTree::xi_new();
        tree.xi_insert(super::Xi99Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi99Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi99_interval_tree_gaps() {
        let mut tree = super::Xi99IntervalTree::xi_new();
        tree.xi_insert(super::Xi99Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi99Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi99Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi99Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi99Interval::xi_new(8, 10));
    }

    #[test]
    fn xi99_interval_tree_merge() {
        let mut tree = super::Xi99IntervalTree::xi_new();
        tree.xi_insert(super::Xi99Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi99Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi99Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi99Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi99Interval::xi_new(10, 15));
    }

    #[test]
    fn xi99_interval_tree_all() {
        let mut tree = super::Xi99IntervalTree::xi_new();
        tree.xi_insert(super::Xi99Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi99Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi99_interval_tree_empty() {
        let tree = super::Xi99IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi99_interval_tree_contains_point() {
        let iv = super::Xi99Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 100) ---

    #[test]
    fn xj_100_uf_make_and_find() {
        let mut uf = super::Xj100UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_100_uf_union_connected() {
        let mut uf = super::Xj100UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_100_uf_component_count() {
        let mut uf = super::Xj100UnionFind::xj_new();
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
    fn xj_100_uf_component_size() {
        let mut uf = super::Xj100UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_100_uf_largest_component() {
        let mut uf = super::Xj100UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_100_uf_many_elements() {
        let mut uf = super::Xj100UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_100_uf_separate_components() {
        let mut uf = super::Xj100UnionFind::xj_new();
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
    fn xj_100_uf_path_compression() {
        let mut uf = super::Xj100UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_100_bt_insert_get() {
        let mut bt = super::Xj100BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_100_bt_contains_len() {
        let mut bt = super::Xj100BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_100_bt_replace() {
        let mut bt = super::Xj100BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_100_bt_remove() {
        let mut bt = super::Xj100BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_100_bt_keys_values() {
        let mut bt = super::Xj100BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_100_bt_range() {
        let mut bt = super::Xj100BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_100_bt_min_max() {
        let mut bt = super::Xj100BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_100_bt_many_inserts() {
        let mut bt = super::Xj100BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_99 segment tree tests ---

    #[test]
    fn xk_99_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk99SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_99_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk99SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_99_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk99SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_99_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk99SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_99_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk99SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_99_st_single_element() {
        let data = vec![42];
        let st = super::Xk99SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_99_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk99SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_99_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk99SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_99 disjoint intervals tests ---

    #[test]
    fn xk_99_di_add_and_count() {
        let mut di = super::Xk99DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_99_di_merge_overlap() {
        let mut di = super::Xk99DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_99_di_contains() {
        let mut di = super::Xk99DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_99_di_remove() {
        let mut di = super::Xk99DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_99_di_covered_length() {
        let mut di = super::Xk99DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_99_di_gaps() {
        let mut di = super::Xk99DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_99_di_merge_adjacent() {
        let mut di = super::Xk99DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_99_di_empty() {
        let di = super::Xk99DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_100_rope_new_empty() {
        let rope = super::Xl100Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_100_rope_from_str() {
        let rope = super::Xl100Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_100_rope_insert_at() {
        let mut rope = super::Xl100Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_100_rope_delete_range() {
        let mut rope = super::Xl100Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_100_rope_char_at() {
        let rope = super::Xl100Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_100_rope_split_concat() {
        let rope = super::Xl100Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_100_rope_line_count() {
        let rope = super::Xl100Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_100_rope_line_at() {
        let rope = super::Xl100Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_100_sa_build_and_search() {
        let sa = super::Xl100SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_100_sa_count() {
        let sa = super::Xl100SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_100_sa_longest_repeated() {
        let sa = super::Xl100SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_100_sa_all_positions() {
        let sa = super::Xl100SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_100_sa_len() {
        let sa = super::Xl100SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_100_sa_empty() {
        let sa = super::Xl100SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_100_rope_slice() {
        let rope = super::Xl100Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_100_sa_search_start() {
        let sa = super::Xl100SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_100_sparse_set_get() {
        let mut m = super::Xm100MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_100_sparse_row_col() {
        let mut m = super::Xm100MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_100_sparse_transpose() {
        let mut m = super::Xm100MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_100_sparse_multiply_vec() {
        let mut m = super::Xm100MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_100_sparse_nnz_density() {
        let mut m = super::Xm100MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_100_sparse_clear() {
        let mut m = super::Xm100MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_100_sparse_overwrite_zero() {
        let mut m = super::Xm100MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_100_tokenizer_basic() {
        let t = super::Xm100Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_100_tokenizer_count() {
        let t = super::Xm100Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_100_tokenizer_unique() {
        let t = super::Xm100Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_100_tokenizer_frequency() {
        let t = super::Xm100Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_100_tokenizer_delimiter() {
        let t = super::Xm100Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_100_tokenizer_whitespace() {
        let t = super::Xm100Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_100_tokenizer_empty() {
        let t = super::Xm100Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 99 ----

    #[test]
    fn xn_99_fenwick_prefix_sum() {
        let mut ft = super::Xn99Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_99_fenwick_range_sum() {
        let mut ft = super::Xn99Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_99_fenwick_point_query() {
        let mut ft = super::Xn99Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_99_fenwick_len() {
        let ft = super::Xn99Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_99_fenwick_multiple_updates() {
        let mut ft = super::Xn99Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_99_fenwick_single_element() {
        let mut ft = super::Xn99Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_99_fenwick_find_kth() {
        let mut ft = super::Xn99Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_99_fenwick_negative_delta() {
        let mut ft = super::Xn99Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 99 ----

    #[test]
    fn xn_99_avl_insert_get() {
        let mut m = super::Xn99AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_99_avl_remove() {
        let mut m = super::Xn99AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_99_avl_in_order() {
        let mut m = super::Xn99AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_99_avl_min_max() {
        let mut m = super::Xn99AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_99_avl_floor_ceiling() {
        let mut m = super::Xn99AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_99_avl_height_balanced() {
        let mut m = super::Xn99AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_99_avl_overwrite() {
        let mut m = super::Xn99AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_99_avl_empty() {
        let m: super::Xn99AVL<i32, i32> = super::Xn99AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
