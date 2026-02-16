//! Glob pattern matching.
//!
//! Wraps the `globset` crate to provide VS Code-compatible glob matching.

use std::fmt;

use globset::{Glob, GlobMatcher, GlobSet, GlobSetBuilder};

/// Error type for glob operations.
#[derive(Debug)]
pub enum GlobError {
    /// A glob pattern failed to compile.
    InvalidPattern(globset::Error),
    /// An empty pattern was supplied where one is required.
    EmptyPattern,
}

impl fmt::Display for GlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlobError::InvalidPattern(e) => write!(f, "invalid glob pattern: {e}"),
            GlobError::EmptyPattern => write!(f, "glob pattern must not be empty"),
        }
    }
}

impl std::error::Error for GlobError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GlobError::InvalidPattern(e) => Some(e),
            GlobError::EmptyPattern => None,
        }
    }
}

impl From<globset::Error> for GlobError {
    fn from(err: globset::Error) -> Self {
        GlobError::InvalidPattern(err)
    }
}

/// A compiled glob pattern for matching file paths.
pub struct GlobPattern {
    matcher: GlobMatcher,
    pattern: String,
}

impl fmt::Debug for GlobPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GlobPattern")
            .field("pattern", &self.pattern)
            .finish()
    }
}

impl fmt::Display for GlobPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pattern)
    }
}

impl GlobPattern {
    /// Compile a glob pattern string.
    pub fn new(pattern: &str) -> Result<Self, globset::Error> {
        let glob = Glob::new(pattern)?;
        Ok(Self {
            matcher: glob.compile_matcher(),
            pattern: pattern.to_string(),
        })
    }

    /// Test if a path matches this pattern.
    pub fn matches(&self, path: &str) -> bool {
        self.matcher.is_match(path)
    }

    /// Get the original pattern string.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Check if a pattern string is negated (starts with `!`).
    pub fn is_negated(pattern: &str) -> bool {
        pattern.starts_with('!')
    }

    /// Strip the leading `!` from a negated pattern, returning the rest.
    /// If the pattern is not negated, returns it unchanged.
    pub fn strip_negation(pattern: &str) -> &str {
        pattern.strip_prefix('!').unwrap_or(pattern)
    }

    /// Compile a glob pattern, returning a [`GlobError`] if the pattern is
    /// empty or invalid.
    pub fn new_validated(pattern: &str) -> Result<Self, GlobError> {
        if pattern.is_empty() {
            return Err(GlobError::EmptyPattern);
        }
        Ok(Self::new(pattern)?)
    }

    /// Returns `true` if the underlying pattern contains glob metacharacters.
    pub fn has_meta(&self) -> bool {
        const META: &[char] = &['*', '?', '[', '{'];
        self.pattern.contains(META)
    }
}

/// A set of glob patterns compiled for efficient matching.
pub struct GlobPatternSet {
    set: GlobSet,
    patterns: Vec<String>,
}

impl fmt::Debug for GlobPatternSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GlobPatternSet")
            .field("patterns", &self.patterns)
            .finish()
    }
}

impl fmt::Display for GlobPatternSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GlobPatternSet({})", self.patterns.join(", "))
    }
}

impl GlobPatternSet {
    /// Build a pattern set from multiple glob strings.
    pub fn new(patterns: &[&str]) -> Result<Self, globset::Error> {
        let mut builder = GlobSetBuilder::new();
        let mut pattern_strings = Vec::new();
        for pat in patterns {
            builder.add(Glob::new(pat)?);
            pattern_strings.push(pat.to_string());
        }
        Ok(Self {
            set: builder.build()?,
            patterns: pattern_strings,
        })
    }

    /// Test if a path matches any pattern in the set.
    pub fn matches_any(&self, path: &str) -> bool {
        self.set.is_match(path)
    }

    /// Get indices of all patterns that match.
    pub fn matching_patterns(&self, path: &str) -> Vec<usize> {
        self.set.matches(path)
    }

    /// Get the pattern strings.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// Return the number of patterns in the set.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Return true if the set contains no patterns.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Return the original pattern strings for every matching pattern.
    pub fn matching_pattern_strings(&self, path: &str) -> Vec<&str> {
        self.set
            .matches(path)
            .into_iter()
            .filter_map(|i| self.patterns.get(i).map(|s| s.as_str()))
            .collect()
    }

    /// Return `true` if the set contains the given pattern string.
    pub fn contains_pattern(&self, pattern: &str) -> bool {
        self.patterns.iter().any(|p| p == pattern)
    }
}

/// Parse a VS Code-style exclude pattern (supports negation with `!`).
pub fn parse_exclude_patterns(patterns: &[&str]) -> (Vec<String>, Vec<String>) {
    let mut includes = Vec::new();
    let mut excludes = Vec::new();
    for pat in patterns {
        if let Some(stripped) = pat.strip_prefix('!') {
            includes.push(stripped.to_string());
        } else {
            excludes.push(pat.to_string());
        }
    }
    (excludes, includes)
}

/// Normalize a glob pattern by converting backslashes to forward slashes.
pub fn normalize_glob_pattern(pattern: &str) -> String {
    pattern.replace('\\', "/")
}

/// Split a glob pattern into a base path and glob part.
///
/// The base path is the longest prefix of literal path segments (no glob
/// metacharacters). Everything from the first segment containing a glob
/// metacharacter onward is the glob part.
///
/// # Examples
/// ```
/// # use vsedit_glob::split_glob_pattern;
/// assert_eq!(split_glob_pattern("src/**/*.rs"), ("src/", "**/*.rs"));
/// assert_eq!(split_glob_pattern("*.rs"), ("", "*.rs"));
/// assert_eq!(split_glob_pattern("src/main.rs"), ("src/main.rs", ""));
/// ```
pub fn split_glob_pattern(pattern: &str) -> (&str, &str) {
    const META: &[char] = &['*', '?', '[', '{'];
    match pattern.find(META) {
        Some(pos) => {
            // Walk back to the last '/' before the metacharacter.
            let base_end = pattern[..pos].rfind('/').map(|i| i + 1).unwrap_or(0);
            (&pattern[..base_end], &pattern[base_end..])
        }
        None => (pattern, ""),
    }
}

/// A file filter that combines include and exclude glob pattern sets.
///
/// A path is accepted when it matches at least one include pattern (or the
/// include set is empty) **and** does not match any exclude pattern.
pub struct FileFilter {
    includes: GlobPatternSet,
    excludes: GlobPatternSet,
}

impl fmt::Debug for FileFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileFilter")
            .field("includes", &self.includes)
            .field("excludes", &self.excludes)
            .finish()
    }
}

/// Builder for constructing a [`FileFilter`] incrementally.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFilterBuilder {
    includes: Vec<String>,
    excludes: Vec<String>,
}

impl FileFilterBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an include pattern.
    pub fn include(mut self, pattern: &str) -> Self {
        self.includes.push(pattern.to_string());
        self
    }

    /// Add an exclude pattern.
    pub fn exclude(mut self, pattern: &str) -> Self {
        self.excludes.push(pattern.to_string());
        self
    }

    /// Add multiple include patterns at once.
    pub fn includes(mut self, patterns: &[&str]) -> Self {
        self.includes.extend(patterns.iter().map(|s| s.to_string()));
        self
    }

    /// Add multiple exclude patterns at once.
    pub fn excludes(mut self, patterns: &[&str]) -> Self {
        self.excludes.extend(patterns.iter().map(|s| s.to_string()));
        self
    }

    /// Compile the builder into a [`FileFilter`].
    pub fn build(self) -> Result<FileFilter, globset::Error> {
        let inc_refs: Vec<&str> = self.includes.iter().map(|s| s.as_str()).collect();
        let exc_refs: Vec<&str> = self.excludes.iter().map(|s| s.as_str()).collect();
        FileFilter::new(&inc_refs, &exc_refs)
    }
}

impl FileFilter {
    /// Create a new `FileFilter` from explicit include and exclude patterns.
    pub fn new(includes: &[&str], excludes: &[&str]) -> Result<Self, globset::Error> {
        Ok(Self {
            includes: GlobPatternSet::new(includes)?,
            excludes: GlobPatternSet::new(excludes)?,
        })
    }

    /// Create a `FileFilter` by auto-separating negated (`!`) patterns.
    ///
    /// Patterns starting with `!` are treated as excludes (after stripping the
    /// prefix); all other patterns are treated as includes.
    pub fn from_patterns(patterns: &[&str]) -> Result<Self, globset::Error> {
        let mut includes = Vec::new();
        let mut excludes = Vec::new();
        for pat in patterns {
            if GlobPattern::is_negated(pat) {
                excludes.push(GlobPattern::strip_negation(pat));
            } else {
                includes.push(*pat);
            }
        }
        Self::new(&includes, &excludes)
    }

    /// Return `true` if `path` is accepted by this filter.
    pub fn accepts(&self, path: &str) -> bool {
        let included = self.includes.is_empty() || self.includes.matches_any(path);
        included && !self.excludes.matches_any(path)
    }

    /// Filter an iterator of paths, returning only accepted paths.
    pub fn filter_paths<'a, I>(&self, paths: I) -> Vec<String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        paths.into_iter().filter(|p| self.accepts(p)).map(|p| p.to_string()).collect()
    }

    /// Return `true` if this filter has no include or exclude patterns.
    pub fn is_empty(&self) -> bool {
        self.includes.is_empty() && self.excludes.is_empty()
    }

    /// Return a new builder for constructing a `FileFilter`.
    pub fn builder() -> FileFilterBuilder {
        FileFilterBuilder::new()
    }
}

/// Validate that all supplied patterns compile successfully.
///
/// Returns `Ok(())` if every pattern is valid, or the first error encountered.
pub fn validate_patterns(patterns: &[&str]) -> Result<(), GlobError> {
    for pat in patterns {
        if pat.is_empty() {
            return Err(GlobError::EmptyPattern);
        }
        Glob::new(pat)?;
    }
    Ok(())
}

/// Expand brace alternatives in a simple glob pattern.
///
/// Given a pattern like `*.{rs,toml}`, returns `["*.rs", "*.toml"]`.
/// If the pattern contains no brace alternatives, returns it unchanged.
pub fn expand_braces(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_string()];
    };
    let Some(close) = pattern[open..].find('}') else {
        return vec![pattern.to_string()];
    };
    let close = open + close;
    let prefix = &pattern[..open];
    let suffix = &pattern[close + 1..];
    let alternatives = &pattern[open + 1..close];
    alternatives
        .split(',')
        .map(|alt| format!("{prefix}{alt}{suffix}"))
        .collect()
}

/// Accumulated statistics for glob operations.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl GlobStats {
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
    pub fn merge(&mut self, other: &GlobStats) {
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

impl Default for GlobStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GlobStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GlobStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for glob.
#[derive(Debug, Clone)]
pub struct GlobValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl GlobValidator {
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

impl Default for GlobValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// A matcher that evaluates a path against multiple categorized glob patterns.
///
/// Each pattern can be tagged as include or exclude, and the matcher reports
/// which categories matched.
pub struct MultiGlobMatcher {
    include_patterns: Vec<(String, GlobMatcher)>,
    exclude_patterns: Vec<(String, GlobMatcher)>,
}

impl MultiGlobMatcher {
    /// Build a new matcher from include and exclude pattern strings.
    pub fn new(includes: &[&str], excludes: &[&str]) -> Result<Self, GlobError> {
        let mut include_patterns = Vec::new();
        for pat in includes {
            if pat.is_empty() {
                return Err(GlobError::EmptyPattern);
            }
            let g = Glob::new(pat)?;
            include_patterns.push((pat.to_string(), g.compile_matcher()));
        }
        let mut exclude_patterns = Vec::new();
        for pat in excludes {
            if pat.is_empty() {
                return Err(GlobError::EmptyPattern);
            }
            let g = Glob::new(pat)?;
            exclude_patterns.push((pat.to_string(), g.compile_matcher()));
        }
        Ok(Self {
            include_patterns,
            exclude_patterns,
        })
    }

    /// Test if a path matches (included and not excluded).
    pub fn matches(&self, path: &str) -> bool {
        let included = self.include_patterns.is_empty()
            || self.include_patterns.iter().any(|(_, m)| m.is_match(path));
        let excluded = self
            .exclude_patterns
            .iter()
            .any(|(_, m)| m.is_match(path));
        included && !excluded
    }

    /// Return patterns that match the given path.
    pub fn matching_includes(&self, path: &str) -> Vec<&str> {
        self.include_patterns
            .iter()
            .filter(|(_, m)| m.is_match(path))
            .map(|(p, _)| p.as_str())
            .collect()
    }

    /// Return exclude patterns that match the given path.
    pub fn matching_excludes(&self, path: &str) -> Vec<&str> {
        self.exclude_patterns
            .iter()
            .filter(|(_, m)| m.is_match(path))
            .map(|(p, _)| p.as_str())
            .collect()
    }

    /// Total number of patterns (include + exclude).
    pub fn pattern_count(&self) -> usize {
        self.include_patterns.len() + self.exclude_patterns.len()
    }
}

impl fmt::Debug for MultiGlobMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiGlobMatcher")
            .field(
                "includes",
                &self
                    .include_patterns
                    .iter()
                    .map(|(p, _)| p.as_str())
                    .collect::<Vec<_>>(),
            )
            .field(
                "excludes",
                &self
                    .exclude_patterns
                    .iter()
                    .map(|(p, _)| p.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Toggle the negation prefix on a glob pattern.
///
/// If the pattern starts with `!`, strip it. Otherwise, prepend `!`.
pub fn negate_pattern(pattern: &str) -> String {
    if let Some(stripped) = pattern.strip_prefix('!') {
        stripped.to_string()
    } else {
        format!("!{pattern}")
    }
}

/// Apply negation to multiple patterns.
pub fn negate_patterns(patterns: &[&str]) -> Vec<String> {
    patterns.iter().map(|p| negate_pattern(p)).collect()
}

/// Convert a simple glob pattern to an equivalent regex string.
///
/// Supports:
/// - `*` → `[^/]*` (match anything except path separator)
/// - `**` → `.*` (match anything including path separator)
/// - `?` → `[^/]` (match single character except path separator)
/// - `.` → `\\.` (literal dot)
/// - All other regex metacharacters are escaped.
pub fn glob_to_regex(pattern: &str) -> String {
    let mut regex = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    // **
                    if i + 2 < chars.len() && chars[i + 2] == '/' {
                        regex.push_str("(.*/)?");
                        i += 3;
                    } else {
                        regex.push_str(".*");
                        i += 2;
                    }
                } else {
                    regex.push_str("[^/]*");
                    i += 1;
                }
            }
            '?' => {
                regex.push_str("[^/]");
                i += 1;
            }
            '.' => {
                regex.push_str("\\.");
                i += 1;
            }
            c @ ('+' | '(' | ')' | '{' | '}' | '[' | ']' | '^' | '$' | '|' | '\\') => {
                regex.push('\\');
                regex.push(c);
                i += 1;
            }
            c => {
                regex.push(c);
                i += 1;
            }
        }
    }
    regex.push('$');
    regex
}

impl GlobPattern {
    /// Extract the file extension from the pattern, if one is present.
    ///
    /// Returns the extension without the leading dot. Only returns a value when
    /// the pattern ends with a literal (non-glob) extension segment.
    pub fn extension(&self) -> Option<&str> {
        let pat = self.pattern.as_str();
        let after_slash = pat.rsplit('/').next().unwrap_or(pat);
        let dot_pos = after_slash.rfind('.')?;
        let ext = &after_slash[dot_pos + 1..];
        if ext.is_empty() {
            return None;
        }
        const META: &[char] = &['*', '?', '[', '{'];
        if ext.contains(META) {
            return None;
        }
        let offset = pat.len() - after_slash.len() + dot_pos + 1;
        Some(&self.pattern[offset..])
    }

    /// Return the non-glob prefix directory of the pattern.
    ///
    /// This is the longest leading path composed entirely of literal segments.
    pub fn base_dir(&self) -> &str {
        split_glob_pattern(&self.pattern).0
    }
}

impl GlobPatternSet {
    /// Merge two pattern sets into a new combined set.
    pub fn merge(&self, other: &GlobPatternSet) -> Result<GlobPatternSet, globset::Error> {
        let combined: Vec<&str> = self
            .patterns
            .iter()
            .chain(other.patterns.iter())
            .map(|s| s.as_str())
            .collect();
        GlobPatternSet::new(&combined)
    }

    /// Iterate over the pattern strings in this set.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.patterns.iter().map(|s| s.as_str())
    }
}

impl<'a> IntoIterator for &'a GlobPatternSet {
    type Item = &'a str;
    type IntoIter = std::iter::Map<std::slice::Iter<'a, String>, fn(&'a String) -> &'a str>;

    fn into_iter(self) -> Self::IntoIter {
        self.patterns.iter().map(|s| s.as_str())
    }
}

impl FileFilter {
    /// Return `true` if the given path matches at least one include pattern.
    pub fn matches(&self, path: &str) -> bool {
        self.includes.matches_any(path)
    }

    /// Return the number of include patterns.
    pub fn included_count(&self) -> usize {
        self.includes.pattern_count()
    }

    /// Return the number of exclude patterns.
    pub fn excluded_count(&self) -> usize {
        self.excludes.pattern_count()
    }
}

/// The category of a glob pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternCategory {
    FileExtension,
    Directory,
    Recursive,
    Literal,
    Complex,
}

/// Categorizes glob patterns by their structure.
#[derive(Debug, Clone, Default)]
pub struct GlobPatternClassifier {
    results: Vec<(String, PatternCategory)>,
}

impl GlobPatternClassifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Classify a single pattern string.
    pub fn classify(pattern: &str) -> PatternCategory {
        if pattern.contains("**") {
            return PatternCategory::Recursive;
        }
        if pattern.ends_with('/') {
            return PatternCategory::Directory;
        }
        const META: &[char] = &['*', '?', '[', '{'];
        if !pattern.contains(META) {
            return PatternCategory::Literal;
        }
        if pattern.starts_with("*.") && !pattern[2..].contains(META) {
            return PatternCategory::FileExtension;
        }
        PatternCategory::Complex
    }

    /// Classify all patterns and store results.
    pub fn classify_all(&mut self, patterns: &[&str]) {
        for pat in patterns {
            self.results
                .push((pat.to_string(), Self::classify(pat)));
        }
    }

    /// Return the stored classification results.
    pub fn results(&self) -> &[(String, PatternCategory)] {
        &self.results
    }

    /// Return only patterns matching a given category.
    pub fn patterns_of(&self, category: PatternCategory) -> Vec<&str> {
        self.results
            .iter()
            .filter(|(_, c)| *c == category)
            .map(|(p, _)| p.as_str())
            .collect()
    }
}

impl GlobStats {
    /// Return the average pattern length for a set of patterns, or `None` if empty.
    pub fn average_pattern_length(patterns: &[String]) -> Option<f64> {
        if patterns.is_empty() {
            return None;
        }
        let total: usize = patterns.iter().map(|p| p.len()).sum();
        Some(total as f64 / patterns.len() as f64)
    }

    /// Return the longest pattern, or `None` if the slice is empty.
    pub fn longest_pattern(patterns: &[String]) -> Option<&str> {
        patterns.iter().max_by_key(|p| p.len()).map(|p| p.as_str())
    }

    /// Return the shortest pattern, or `None` if the slice is empty.
    pub fn shortest_pattern(patterns: &[String]) -> Option<&str> {
        patterns.iter().min_by_key(|p| p.len()).map(|p| p.as_str())
    }
}

/// Result of analyzing a glob pattern's structural properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobPatternAnalysis {
    /// The original pattern string.
    pub pattern: String,
    /// Whether the pattern contains `**` (recursive matching).
    pub is_recursive: bool,
    /// Whether the pattern contains any wildcard characters (`*`, `?`).
    pub has_wildcards: bool,
    /// Whether the pattern contains character classes (`[...]`).
    pub has_char_class: bool,
    /// Whether the pattern contains brace alternatives (`{a,b}`).
    pub has_braces: bool,
    /// The longest literal directory prefix before any glob metacharacter.
    pub base_path: String,
    /// The estimated directory depth of the pattern (number of `/` separators).
    pub depth: usize,
}

impl GlobPatternAnalysis {
    /// Analyze a glob pattern string and return its structural properties.
    pub fn analyze(pattern: &str) -> Self {
        let (base, _) = split_glob_pattern(pattern);
        Self {
            pattern: pattern.to_string(),
            is_recursive: pattern.contains("**"),
            has_wildcards: pattern.contains('*') || pattern.contains('?'),
            has_char_class: pattern.contains('['),
            has_braces: pattern.contains('{'),
            base_path: base.to_string(),
            depth: pattern.chars().filter(|&c| c == '/').count(),
        }
    }

    /// Return `true` if the pattern is a plain literal (no metacharacters).
    pub fn is_literal(&self) -> bool {
        !self.has_wildcards && !self.has_char_class && !self.has_braces
    }
}

/// Fully normalize a glob pattern for consistent matching.
///
/// Performs the following transformations:
/// - Convert backslashes to forward slashes.
/// - Strip a leading `./` prefix.
/// - Collapse consecutive slashes into a single slash.
/// - Remove a trailing slash (unless the pattern is `/`).
pub fn normalize_pattern(pattern: &str) -> String {
    let mut s = pattern.replace('\\', "/");

    // Strip leading "./"
    while s.starts_with("./") {
        s = s[2..].to_string();
    }

    // Collapse consecutive slashes.
    while s.contains("//") {
        s = s.replace("//", "/");
    }

    // Remove trailing slash unless the entire string is "/".
    if s.len() > 1 && s.ends_with('/') {
        s.pop();
    }

    s
}

/// Match a single compiled [`GlobPattern`] against many paths at once.
///
/// Returns the subset of paths that match the pattern, preserving order.
pub fn batch_match<'a>(pattern: &GlobPattern, paths: &[&'a str]) -> Vec<&'a str> {
    paths
        .iter()
        .copied()
        .filter(|p| pattern.matches(p))
        .collect()
}

/// Match a single compiled [`GlobPattern`] against many paths, returning
/// a parallel `Vec<bool>` indicating which paths matched.
pub fn batch_match_flags(pattern: &GlobPattern, paths: &[&str]) -> Vec<bool> {
    paths.iter().map(|p| pattern.matches(p)).collect()
}

/// A rule in a [`GlobFilterChain`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterAction {
    /// Include paths matching this rule.
    Include,
    /// Exclude paths matching this rule.
    Exclude,
}

/// An ordered chain of include/exclude glob rules evaluated top-to-bottom.
///
/// Rules are evaluated in insertion order. The *last* matching rule wins.
/// If no rule matches, the path is accepted by default.
pub struct GlobFilterChain {
    rules: Vec<(FilterAction, GlobMatcher, String)>,
}

impl fmt::Debug for GlobFilterChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entries: Vec<_> = self
            .rules
            .iter()
            .map(|(action, _, pat)| (*action, pat.as_str()))
            .collect();
        f.debug_struct("GlobFilterChain")
            .field("rules", &entries)
            .finish()
    }
}

impl GlobFilterChain {
    /// Create a new empty filter chain.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Append a rule to the chain.
    pub fn add_rule(
        &mut self,
        action: FilterAction,
        pattern: &str,
    ) -> Result<(), globset::Error> {
        let glob = Glob::new(pattern)?;
        self.rules
            .push((action, glob.compile_matcher(), pattern.to_string()));
        Ok(())
    }

    /// Evaluate the chain for a given path.
    ///
    /// Returns `true` (accepted) if the last matching rule is [`FilterAction::Include`]
    /// or if no rule matched at all. Returns `false` if the last matching rule
    /// is [`FilterAction::Exclude`].
    pub fn accepts(&self, path: &str) -> bool {
        let mut result = true;
        for (action, matcher, _) in &self.rules {
            if matcher.is_match(path) {
                result = *action == FilterAction::Include;
            }
        }
        result
    }

    /// Filter an iterator of paths through the chain.
    pub fn filter_paths<'a>(&self, paths: &[&'a str]) -> Vec<&'a str> {
        paths.iter().copied().filter(|p| self.accepts(p)).collect()
    }

    /// Return the number of rules in the chain.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Return `true` if the chain contains no rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

impl Default for GlobFilterChain {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobPatternSet {
    /// Return a new set containing only patterns from `self` that also appear
    /// in `other` (by pattern string equality).
    pub fn intersection(&self, other: &GlobPatternSet) -> Result<GlobPatternSet, globset::Error> {
        let common: Vec<&str> = self
            .patterns
            .iter()
            .filter(|p| other.patterns.iter().any(|o| o == *p))
            .map(|s| s.as_str())
            .collect();
        GlobPatternSet::new(&common)
    }

    /// Return a new set containing patterns from `self` that do **not** appear
    /// in `other` (by pattern string equality).
    pub fn difference(&self, other: &GlobPatternSet) -> Result<GlobPatternSet, globset::Error> {
        let diff: Vec<&str> = self
            .patterns
            .iter()
            .filter(|p| !other.patterns.iter().any(|o| o == *p))
            .map(|s| s.as_str())
            .collect();
        GlobPatternSet::new(&diff)
    }
}

/// Check whether a pattern contains only valid glob syntax characters.
pub fn is_valid_glob_syntax(pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    let mut brace_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    for ch in pattern.chars() {
        match ch {
            '{' => brace_depth += 1,
            '}' => {
                brace_depth -= 1;
                if brace_depth < 0 {
                    return false;
                }
            }
            '[' => bracket_depth += 1,
            ']' => {
                bracket_depth -= 1;
                if bracket_depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    brace_depth == 0 && bracket_depth == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        let pat = GlobPattern::new("*.rs").unwrap();
        assert!(pat.matches("main.rs"));
        assert!(!pat.matches("main.ts"));
    }

    #[test]
    fn test_glob_set() {
        let set = GlobPatternSet::new(&["*.rs", "*.toml"]).unwrap();
        assert!(set.matches_any("Cargo.toml"));
        assert!(set.matches_any("main.rs"));
        assert!(!set.matches_any("main.ts"));
    }

    #[test]
    fn test_exclude_patterns() {
        let (excludes, includes) = parse_exclude_patterns(&["node_modules", "!important.js"]);
        assert_eq!(excludes, vec!["node_modules"]);
        assert_eq!(includes, vec!["important.js"]);
    }

    #[test]
    fn test_is_negated() {
        assert!(GlobPattern::is_negated("!*.log"));
        assert!(!GlobPattern::is_negated("*.log"));
        assert!(!GlobPattern::is_negated(""));
    }

    #[test]
    fn test_strip_negation() {
        assert_eq!(GlobPattern::strip_negation("!*.log"), "*.log");
        assert_eq!(GlobPattern::strip_negation("*.rs"), "*.rs");
        assert_eq!(GlobPattern::strip_negation("!"), "");
    }

    #[test]
    fn test_pattern_set_count_and_empty() {
        let empty = GlobPatternSet::new(&[]).unwrap();
        assert!(empty.is_empty());
        assert_eq!(empty.pattern_count(), 0);

        let set = GlobPatternSet::new(&["*.rs", "*.toml"]).unwrap();
        assert!(!set.is_empty());
        assert_eq!(set.pattern_count(), 2);
    }

    #[test]
    fn test_normalize_glob_pattern() {
        assert_eq!(normalize_glob_pattern("src\\**\\*.rs"), "src/**/*.rs");
        assert_eq!(normalize_glob_pattern("already/fine"), "already/fine");
        assert_eq!(normalize_glob_pattern(""), "");
    }

    #[test]
    fn test_split_glob_pattern() {
        assert_eq!(split_glob_pattern("src/**/*.rs"), ("src/", "**/*.rs"));
        assert_eq!(split_glob_pattern("*.rs"), ("", "*.rs"));
        assert_eq!(split_glob_pattern("src/main.rs"), ("src/main.rs", ""));
        assert_eq!(split_glob_pattern("a/b/c/*.txt"), ("a/b/c/", "*.txt"));
        assert_eq!(split_glob_pattern("{a,b}"), ("", "{a,b}"));
    }

    #[test]
    fn test_file_filter_accepts() {
        let filter = FileFilter::new(&["*.rs"], &["test_*.rs"]).unwrap();
        assert!(filter.accepts("main.rs"));
        assert!(!filter.accepts("test_main.rs"));
        assert!(!filter.accepts("readme.md"));
    }

    #[test]
    fn test_file_filter_empty_includes() {
        let filter = FileFilter::new(&[], &["*.log"]).unwrap();
        assert!(filter.accepts("main.rs"));
        assert!(!filter.accepts("debug.log"));
    }

    #[test]
    fn test_file_filter_from_patterns() {
        let filter = FileFilter::from_patterns(&["*.rs", "!test_*.rs"]).unwrap();
        assert!(filter.accepts("lib.rs"));
        assert!(!filter.accepts("test_lib.rs"));
        assert!(!filter.accepts("readme.md"));
    }

    // --- New tests ---

    #[test]
    fn test_glob_error_display() {
        let err = GlobError::EmptyPattern;
        assert_eq!(err.to_string(), "glob pattern must not be empty");

        let err2 = GlobPattern::new_validated("").unwrap_err();
        assert!(err2.to_string().contains("empty"));
    }

    #[test]
    fn test_new_validated_empty() {
        assert!(matches!(
            GlobPattern::new_validated(""),
            Err(GlobError::EmptyPattern)
        ));
    }

    #[test]
    fn test_new_validated_ok() {
        let pat = GlobPattern::new_validated("*.rs").unwrap();
        assert!(pat.matches("lib.rs"));
    }

    #[test]
    fn test_has_meta() {
        let star = GlobPattern::new("*.rs").unwrap();
        assert!(star.has_meta());

        let literal = GlobPattern::new("Cargo.toml").unwrap();
        assert!(!literal.has_meta());
    }

    #[test]
    fn test_glob_pattern_display_debug() {
        let pat = GlobPattern::new("src/**/*.rs").unwrap();
        assert_eq!(format!("{pat}"), "src/**/*.rs");
        assert!(format!("{pat:?}").contains("src/**/*.rs"));
    }

    #[test]
    fn test_pattern_set_display_debug() {
        let set = GlobPatternSet::new(&["*.rs", "*.toml"]).unwrap();
        let display = format!("{set}");
        assert!(display.contains("*.rs"));
        assert!(display.contains("*.toml"));
        assert!(format!("{set:?}").contains("GlobPatternSet"));
    }

    #[test]
    fn test_matching_pattern_strings() {
        let set = GlobPatternSet::new(&["*.rs", "*.toml", "src/**"]).unwrap();
        let matched = set.matching_pattern_strings("src/lib.rs");
        assert!(matched.contains(&"*.rs"));
        assert!(matched.contains(&"src/**"));
        assert!(!matched.contains(&"*.toml"));
    }

    #[test]
    fn test_contains_pattern() {
        let set = GlobPatternSet::new(&["*.rs", "*.toml"]).unwrap();
        assert!(set.contains_pattern("*.rs"));
        assert!(!set.contains_pattern("*.py"));
    }

    #[test]
    fn test_file_filter_builder() {
        let filter = FileFilter::builder()
            .include("*.rs")
            .include("*.toml")
            .exclude("test_*")
            .build()
            .unwrap();
        assert!(filter.accepts("main.rs"));
        assert!(filter.accepts("Cargo.toml"));
        assert!(!filter.accepts("test_main.rs"));
        assert!(!filter.accepts("readme.md"));
    }

    #[test]
    fn test_file_filter_builder_batch() {
        let filter = FileFilter::builder()
            .includes(&["*.rs", "*.toml"])
            .excludes(&["test_*", "bench_*"])
            .build()
            .unwrap();
        assert!(filter.accepts("lib.rs"));
        assert!(!filter.accepts("bench_sort.rs"));
    }

    #[test]
    fn test_filter_paths() {
        let filter = FileFilter::new(&["*.rs"], &["test_*.rs"]).unwrap();
        let paths = vec!["main.rs", "test_main.rs", "lib.rs", "readme.md"];
        let accepted = filter.filter_paths(paths.into_iter());
        assert_eq!(accepted, vec!["main.rs", "lib.rs"]);
    }

    #[test]
    fn test_file_filter_is_empty() {
        let empty = FileFilter::new(&[], &[]).unwrap();
        assert!(empty.is_empty());

        let non_empty = FileFilter::new(&["*.rs"], &[]).unwrap();
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_validate_patterns_ok() {
        assert!(validate_patterns(&["*.rs", "src/**/*.toml"]).is_ok());
    }

    #[test]
    fn test_validate_patterns_empty() {
        let err = validate_patterns(&["*.rs", ""]).unwrap_err();
        assert!(matches!(err, GlobError::EmptyPattern));
    }

    #[test]
    fn test_expand_braces_single() {
        let expanded = expand_braces("*.{rs,toml}");
        assert_eq!(expanded, vec!["*.rs", "*.toml"]);
    }

    #[test]
    fn test_expand_braces_no_braces() {
        let expanded = expand_braces("*.rs");
        assert_eq!(expanded, vec!["*.rs"]);
    }

    #[test]
    fn test_expand_braces_three_alternatives() {
        let expanded = expand_braces("src/*.{rs,toml,lock}");
        assert_eq!(expanded, vec!["src/*.rs", "src/*.toml", "src/*.lock"]);
    }

    #[test]
    fn test_file_filter_debug() {
        let filter = FileFilter::new(&["*.rs"], &["test_*"]).unwrap();
        let dbg = format!("{filter:?}");
        assert!(dbg.contains("FileFilter"));
        assert!(dbg.contains("*.rs"));
    }

    #[test]
    fn glob_stats_new_defaults() {
        let stats = GlobStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn glob_stats_record_success() {
        let mut stats = GlobStats::new();
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
    fn glob_stats_record_failure() {
        let mut stats = GlobStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn glob_stats_reset() {
        let mut stats = GlobStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn glob_stats_merge() {
        let mut a = GlobStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = GlobStats::new();
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
    fn glob_stats_display() {
        let mut stats = GlobStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn glob_stats_default() {
        let stats = GlobStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn glob_validator_accepts_valid_name() {
        let v = GlobValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn glob_validator_rejects_empty() {
        let v = GlobValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn glob_validator_rejects_too_long() {
        let v = GlobValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn glob_validator_forbidden_prefix() {
        let v = GlobValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn glob_validator_allowed_chars() {
        let v = GlobValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn glob_validator_range() {
        let v = GlobValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn glob_sanitize_removes_control() {
        let result = GlobValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn glob_truncate_short_string() {
        assert_eq!(GlobValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn glob_truncate_long_string() {
        let result = GlobValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn glob_is_ascii_printable() {
        assert!(GlobValidator::is_ascii_printable("Hello World 123"));
        assert!(!GlobValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn multi_glob_matcher_basic() {
        let m = MultiGlobMatcher::new(&["*.rs", "*.toml"], &["test_*"]).unwrap();
        assert!(m.matches("main.rs"));
        assert!(!m.matches("test_main.rs"));
        assert!(!m.matches("readme.md"));
    }

    #[test]
    fn multi_glob_matcher_matching_includes() {
        let m = MultiGlobMatcher::new(&["*.rs", "src/**"], &[]).unwrap();
        let matched = m.matching_includes("src/lib.rs");
        assert!(matched.contains(&"*.rs"));
        assert!(matched.contains(&"src/**"));
    }

    #[test]
    fn multi_glob_matcher_matching_excludes() {
        let m = MultiGlobMatcher::new(&["*.rs"], &["test_*", "bench_*"]).unwrap();
        let excludes = m.matching_excludes("test_main.rs");
        assert_eq!(excludes, vec!["test_*"]);
    }

    #[test]
    fn multi_glob_matcher_empty_includes_accepts_all() {
        let m = MultiGlobMatcher::new(&[], &["*.log"]).unwrap();
        assert!(m.matches("main.rs"));
        assert!(!m.matches("debug.log"));
    }

    #[test]
    fn negate_pattern_toggle() {
        assert_eq!(negate_pattern("*.rs"), "!*.rs");
        assert_eq!(negate_pattern("!*.rs"), "*.rs");
        assert_eq!(negate_pattern("!"), "");
    }

    #[test]
    fn negate_patterns_batch() {
        let negated = negate_patterns(&["*.rs", "!*.toml"]);
        assert_eq!(negated, vec!["!*.rs", "*.toml"]);
    }

    #[test]
    fn glob_to_regex_star() {
        let re = glob_to_regex("*.rs");
        assert_eq!(re, "^[^/]*\\.rs$");
    }

    #[test]
    fn glob_to_regex_doublestar() {
        let re = glob_to_regex("src/**/*.rs");
        assert_eq!(re, "^src/(.*/)?[^/]*\\.rs$");
    }

    #[test]
    fn glob_pattern_extension() {
        let pat = GlobPattern::new("*.rs").unwrap();
        assert_eq!(pat.extension(), Some("rs"));

        let pat2 = GlobPattern::new("src/**/*.toml").unwrap();
        assert_eq!(pat2.extension(), Some("toml"));

        let pat3 = GlobPattern::new("src/**/*").unwrap();
        assert_eq!(pat3.extension(), None);

        let pat4 = GlobPattern::new("Makefile").unwrap();
        assert_eq!(pat4.extension(), None);

        let pat5 = GlobPattern::new("*.tar.gz").unwrap();
        assert_eq!(pat5.extension(), Some("gz"));
    }

    #[test]
    fn glob_pattern_base_dir() {
        let pat = GlobPattern::new("src/**/*.rs").unwrap();
        assert_eq!(pat.base_dir(), "src/");

        let pat2 = GlobPattern::new("*.rs").unwrap();
        assert_eq!(pat2.base_dir(), "");

        let pat3 = GlobPattern::new("a/b/c.txt").unwrap();
        assert_eq!(pat3.base_dir(), "a/b/c.txt");
    }

    #[test]
    fn glob_pattern_set_merge() {
        let a = GlobPatternSet::new(&["*.rs"]).unwrap();
        let b = GlobPatternSet::new(&["*.toml", "*.lock"]).unwrap();
        let merged = a.merge(&b).unwrap();
        assert_eq!(merged.pattern_count(), 3);
        assert!(merged.matches_any("lib.rs"));
        assert!(merged.matches_any("Cargo.toml"));
        assert!(merged.matches_any("Cargo.lock"));
        assert!(!merged.matches_any("readme.md"));
    }

    #[test]
    fn glob_pattern_set_iter_and_into_iter() {
        let set = GlobPatternSet::new(&["*.rs", "*.toml"]).unwrap();
        let via_iter: Vec<&str> = set.iter().collect();
        assert_eq!(via_iter, vec!["*.rs", "*.toml"]);

        let via_into: Vec<&str> = (&set).into_iter().collect();
        assert_eq!(via_into, vec!["*.rs", "*.toml"]);
    }

    #[test]
    fn file_filter_matches_and_counts() {
        let filter = FileFilter::new(&["*.rs", "*.toml"], &["test_*"]).unwrap();
        assert!(filter.matches("main.rs"));
        assert!(!filter.matches("readme.md"));
        assert_eq!(filter.included_count(), 2);
        assert_eq!(filter.excluded_count(), 1);
    }

    #[test]
    fn glob_pattern_classifier_categories() {
        assert_eq!(
            GlobPatternClassifier::classify("*.rs"),
            PatternCategory::FileExtension
        );
        assert_eq!(
            GlobPatternClassifier::classify("src/**/*.rs"),
            PatternCategory::Recursive
        );
        assert_eq!(
            GlobPatternClassifier::classify("build/"),
            PatternCategory::Directory
        );
        assert_eq!(
            GlobPatternClassifier::classify("Makefile"),
            PatternCategory::Literal
        );
        assert_eq!(
            GlobPatternClassifier::classify("src/*.rs"),
            PatternCategory::Complex
        );

        let mut c = GlobPatternClassifier::new();
        c.classify_all(&["*.rs", "src/**", "Makefile"]);
        assert_eq!(c.results().len(), 3);
        assert_eq!(
            c.patterns_of(PatternCategory::Literal),
            vec!["Makefile"]
        );
    }

    #[test]
    fn glob_stats_pattern_helpers() {
        let patterns: Vec<String> = vec!["*.rs".into(), "src/**/*.toml".into(), "a".into()];
        let avg = GlobStats::average_pattern_length(&patterns).unwrap();
        assert!((avg - 6.0).abs() < f64::EPSILON);
        assert_eq!(GlobStats::longest_pattern(&patterns), Some("src/**/*.toml"));
        assert_eq!(GlobStats::shortest_pattern(&patterns), Some("a"));

        let empty: Vec<String> = vec![];
        assert_eq!(GlobStats::average_pattern_length(&empty), None);
        assert_eq!(GlobStats::longest_pattern(&empty), None);
        assert_eq!(GlobStats::shortest_pattern(&empty), None);
    }

    #[test]
    fn is_valid_glob_syntax_checks() {
        assert!(is_valid_glob_syntax("*.rs"));
        assert!(is_valid_glob_syntax("src/{a,b}/*.rs"));
        assert!(is_valid_glob_syntax("[abc].txt"));
        assert!(!is_valid_glob_syntax(""));
        assert!(!is_valid_glob_syntax("*.{rs"));
        assert!(!is_valid_glob_syntax("foo}bar"));
        assert!(!is_valid_glob_syntax("]bad"));
    }

    #[test]
    fn pattern_analysis_recursive() {
        let a = GlobPatternAnalysis::analyze("src/**/*.rs");
        assert!(a.is_recursive);
        assert!(a.has_wildcards);
        assert!(!a.has_char_class);
        assert!(!a.has_braces);
        assert_eq!(a.base_path, "src/");
        assert_eq!(a.depth, 2);
        assert!(!a.is_literal());
    }

    #[test]
    fn pattern_analysis_literal() {
        let a = GlobPatternAnalysis::analyze("Makefile");
        assert!(!a.is_recursive);
        assert!(!a.has_wildcards);
        assert!(a.is_literal());
        assert_eq!(a.base_path, "Makefile");
        assert_eq!(a.depth, 0);
    }

    #[test]
    fn normalize_pattern_full() {
        assert_eq!(normalize_pattern("src\\\\lib.rs"), "src/lib.rs");
        assert_eq!(normalize_pattern("./src/lib.rs"), "src/lib.rs");
        assert_eq!(normalize_pattern("src//lib.rs"), "src/lib.rs");
        assert_eq!(normalize_pattern("src/lib/"), "src/lib");
        assert_eq!(normalize_pattern("././foo"), "foo");
        assert_eq!(normalize_pattern("/"), "/");
    }

    #[test]
    fn batch_match_paths() {
        let pat = GlobPattern::new("*.rs").unwrap();
        let paths = &["main.rs", "lib.rs", "Cargo.toml", "README.md"];
        let matched = batch_match(&pat, paths);
        assert_eq!(matched, vec!["main.rs", "lib.rs"]);

        let flags = batch_match_flags(&pat, paths);
        assert_eq!(flags, vec![true, true, false, false]);
    }

    #[test]
    fn filter_chain_last_rule_wins() {
        let mut chain = GlobFilterChain::new();
        chain.add_rule(FilterAction::Exclude, "*.rs").unwrap();
        chain.add_rule(FilterAction::Include, "main.rs").unwrap();
        // main.rs matches both rules; last wins (Include)
        assert!(chain.accepts("main.rs"));
        // lib.rs matches only the Exclude rule
        assert!(!chain.accepts("lib.rs"));
        // readme.md matches nothing, default is accept
        assert!(chain.accepts("readme.md"));
        assert_eq!(chain.len(), 2);
        assert!(!chain.is_empty());

        let paths = &["main.rs", "lib.rs", "readme.md"];
        let kept = chain.filter_paths(paths);
        assert_eq!(kept, vec!["main.rs", "readme.md"]);
    }

    #[test]
    fn pattern_set_intersection_and_difference() {
        let a = GlobPatternSet::new(&["*.rs", "*.toml", "*.lock"]).unwrap();
        let b = GlobPatternSet::new(&["*.toml", "*.lock", "*.md"]).unwrap();

        let inter = a.intersection(&b).unwrap();
        assert_eq!(inter.pattern_count(), 2);
        assert!(inter.contains_pattern("*.toml"));
        assert!(inter.contains_pattern("*.lock"));
        assert!(!inter.contains_pattern("*.rs"));

        let diff = a.difference(&b).unwrap();
        assert_eq!(diff.pattern_count(), 1);
        assert!(diff.contains_pattern("*.rs"));
        assert!(!diff.contains_pattern("*.toml"));
    }
}
