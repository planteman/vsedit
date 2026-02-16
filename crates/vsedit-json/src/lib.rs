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
}
