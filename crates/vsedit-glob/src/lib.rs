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

/// Classify a glob pattern into a category describing its complexity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobComplexity {
    /// A literal path with no metacharacters.
    Literal,
    /// A simple pattern with only `*` or `?`.
    Simple,
    /// A pattern using character classes (`[...]`).
    CharacterClass,
    /// A pattern using alternation (`{a,b}`).
    Alternation,
    /// A recursive glob (`**`).
    Recursive,
}

/// Classify the complexity of a glob pattern string.
pub fn classify_glob(pattern: &str) -> GlobComplexity {
    if pattern.contains("**") {
        return GlobComplexity::Recursive;
    }
    if pattern.contains('{') {
        return GlobComplexity::Alternation;
    }
    if pattern.contains('[') {
        return GlobComplexity::CharacterClass;
    }
    if pattern.contains('*') || pattern.contains('?') {
        return GlobComplexity::Simple;
    }
    GlobComplexity::Literal
}

/// Extract the file extension targeted by a glob pattern, if it ends with a
/// literal extension like `*.rs` or `**/*.toml`.
pub fn extract_extension_from_glob(pattern: &str) -> Option<&str> {
    let trimmed = pattern.trim();
    // Look for the last segment after `/` or the whole string
    let last_segment = trimmed.rsplit('/').next().unwrap_or(trimmed);
    // Must start with `*` and have a dot
    if let Some(rest) = last_segment.strip_prefix("*.") {
        if !rest.is_empty() && !rest.contains('*') && !rest.contains('?') && !rest.contains('[') {
            return Some(rest);
        }
    }
    if let Some(rest) = last_segment.strip_prefix("**.") {
        if !rest.is_empty() && !rest.contains('*') && !rest.contains('?') && !rest.contains('[') {
            return Some(rest);
        }
    }
    None
}

/// Return the common prefix of a set of glob pattern strings.
///
/// This is the longest leading substring shared by all patterns, useful for
/// determining a base directory for a search.
pub fn common_glob_prefix(patterns: &[&str]) -> String {
    if patterns.is_empty() {
        return String::new();
    }
    let first = patterns[0];
    let mut prefix_len = first.len();
    for p in &patterns[1..] {
        prefix_len = prefix_len.min(p.len());
        for (i, (a, b)) in first.chars().zip(p.chars()).enumerate() {
            if a != b || i >= prefix_len {
                prefix_len = i;
                break;
            }
        }
    }
    // Trim back to last `/` to keep directory boundary
    let prefix = &first[..prefix_len];
    match prefix.rfind('/') {
        Some(i) => prefix[..=i].to_string(),
        None => String::new(),
    }
}

impl GlobPatternSet {
    /// Return a new set containing only patterns that match a given path.
    pub fn matching_subset(&self, path: &str) -> Result<GlobPatternSet, globset::Error> {
        let matching: Vec<&str> = self
            .set
            .matches(path)
            .into_iter()
            .filter_map(|i| self.patterns.get(i).map(|s| s.as_str()))
            .collect();
        GlobPatternSet::new(&matching)
    }

    /// Return the complexity classification of each pattern in the set.
    pub fn complexities(&self) -> Vec<GlobComplexity> {
        self.patterns.iter().map(|p| classify_glob(p)).collect()
    }
}

impl FileFilter {
    /// Return the include and exclude pattern counts.
    pub fn pattern_counts(&self) -> (usize, usize) {
        (self.includes.pattern_count(), self.excludes.pattern_count())
    }

    /// Return true if this filter has no include or exclude patterns.
    pub fn is_passthrough(&self) -> bool {
        self.includes.is_empty() && self.excludes.is_empty()
    }
}

impl FileFilterBuilder {
    /// Return the number of include patterns added so far.
    pub fn include_count(&self) -> usize {
        self.includes.len()
    }

    /// Return the number of exclude patterns added so far.
    pub fn exclude_count(&self) -> usize {
        self.excludes.len()
    }

    /// Return true if no patterns have been added.
    pub fn is_empty(&self) -> bool {
        self.includes.is_empty() && self.excludes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ExplainerComplexity
// ---------------------------------------------------------------------------

/// Describes how complex a glob pattern is from a human-readability perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplainerComplexity {
    /// A pattern with at most one wildcard segment (e.g. `*.rs`).
    Simple,
    /// A pattern with multiple wildcard segments or character classes.
    Moderate,
    /// A pattern with recursive globs, alternations, or nested braces.
    Complex,
}

impl fmt::Display for ExplainerComplexity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExplainerComplexity::Simple => write!(f, "Simple"),
            ExplainerComplexity::Moderate => write!(f, "Moderate"),
            ExplainerComplexity::Complex => write!(f, "Complex"),
        }
    }
}

// ---------------------------------------------------------------------------
// GlobOptimizer
// ---------------------------------------------------------------------------

/// Collects glob patterns and removes redundant ones.
///
/// A pattern is considered redundant when a broader pattern already covers it.
/// For example, `*.rs` covers `main.rs`, so if both are present `main.rs` is
/// dropped.
#[derive(Debug, Clone)]
pub struct GlobOptimizer {
    patterns: Vec<String>,
}

impl GlobOptimizer {
    /// Create a new, empty optimizer.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    /// Add a pattern to the optimizer.
    pub fn add_pattern(&mut self, pattern: &str) {
        self.patterns.push(pattern.to_string());
    }

    /// Return the number of patterns currently tracked.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Return `true` if any pattern is a subset of another.
    pub fn has_redundant(&self) -> bool {
        for (i, a) in self.patterns.iter().enumerate() {
            for (j, b) in self.patterns.iter().enumerate() {
                if i != j && Self::covers(b, a) {
                    return true;
                }
            }
        }
        false
    }

    /// Return an optimized list with redundant patterns removed.
    ///
    /// A pattern is removed when another pattern in the set already covers it
    /// (i.e. every path matched by the narrower pattern is also matched by the
    /// broader one).
    pub fn optimize(&self) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        for (i, candidate) in self.patterns.iter().enumerate() {
            let dominated = self.patterns.iter().enumerate().any(|(j, other)| {
                i != j && Self::covers(other, candidate)
            });
            if !dominated {
                if !result.contains(candidate) {
                    result.push(candidate.clone());
                }
            }
        }
        result
    }

    /// Returns `true` when `broad` covers every path that `narrow` would match.
    ///
    /// This uses a simple heuristic:
    /// - `**` covers everything.
    /// - `*.ext` covers any literal filename ending with `.ext`.
    /// - `dir/**` covers `dir/sub/**` and `dir/file`.
    /// - `**/*.ext` covers `*.ext` and any `path/*.ext`.
    /// - Identical patterns cover each other.
    fn covers(broad: &str, narrow: &str) -> bool {
        if broad == narrow {
            return false; // identical – not "covered", just duplicate
        }
        if broad == "**" || broad == "**/*" {
            return true;
        }
        // `*.ext` covers a literal filename with that extension
        if let Some(ext) = broad.strip_prefix("*.") {
            if !narrow.contains('*') && !narrow.contains('?') {
                if narrow.ends_with(&format!(".{ext}")) && !narrow.contains('/') {
                    return true;
                }
            }
        }
        // `**/*.ext` covers `*.ext` and `any/path/*.ext`
        if let Some(suffix) = broad.strip_prefix("**/") {
            if narrow == suffix {
                return true;
            }
            if narrow.ends_with(suffix) {
                return true;
            }
        }
        // `dir/**` covers `dir/anything`
        if let Some(prefix) = broad.strip_suffix("/**") {
            if narrow.starts_with(prefix) && narrow.len() > prefix.len() {
                let rest = &narrow[prefix.len()..];
                if rest.starts_with('/') {
                    return true;
                }
            }
        }
        false
    }
}

impl Default for GlobOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// GlobExplainer
// ---------------------------------------------------------------------------

/// Provides human-readable descriptions of glob patterns.
pub struct GlobExplainer;

impl GlobExplainer {
    /// Return a human-readable explanation of `pattern`.
    pub fn explain(pattern: &str) -> String {
        if pattern == "**" || pattern == "**/*" {
            return "Matches all files recursively".to_string();
        }
        if let Some(ext) = pattern.strip_prefix("**/*.") {
            return format!("Matches .{ext} files in any subdirectory");
        }
        if let Some(dir) = pattern.strip_suffix("/**") {
            return format!("Matches everything inside {dir}/");
        }
        if let Some(ext) = pattern.strip_prefix("*.") {
            if !ext.contains('*') && !ext.contains('?') {
                return format!("Matches any file ending with .{ext}");
            }
        }
        if !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[') {
            return format!("Matches the literal path \"{pattern}\"");
        }
        if pattern.contains("**") {
            return format!("Matches paths matching the recursive pattern \"{pattern}\"");
        }
        format!("Matches paths matching \"{pattern}\"")
    }

    /// Classify the complexity of `pattern`.
    pub fn complexity(pattern: &str) -> ExplainerComplexity {
        let has_doublestar = pattern.contains("**");
        let has_braces = pattern.contains('{');
        let has_brackets = pattern.contains('[');
        let single_wildcards = pattern.matches('*').count()
            - pattern.matches("**").count() * 2;
        let question_marks = pattern.matches('?').count();

        if has_braces || (has_doublestar && (has_brackets || single_wildcards > 0)) {
            return ExplainerComplexity::Complex;
        }
        if has_doublestar || has_brackets || single_wildcards + question_marks > 1 {
            return ExplainerComplexity::Moderate;
        }
        ExplainerComplexity::Simple
    }
}

// ---------------------------------------------------------------------------
// GlobNegation
// ---------------------------------------------------------------------------

/// Manages include / exclude pattern lists using simple string matching.
///
/// A path is considered *included* when it matches at least one include
/// pattern **and** does not match any exclude pattern.  Matching is performed
/// with simple heuristics (suffix / contains checks) so that patterns do not
/// need to be valid glob syntax.
#[derive(Debug, Clone)]
pub struct GlobNegation {
    includes: Vec<String>,
    excludes: Vec<String>,
}

impl GlobNegation {
    /// Create a new, empty negation filter.
    pub fn new() -> Self {
        Self {
            includes: Vec::new(),
            excludes: Vec::new(),
        }
    }

    /// Add an include pattern.
    pub fn add_include(&mut self, pattern: &str) {
        self.includes.push(pattern.to_string());
    }

    /// Add an exclude pattern.
    pub fn add_exclude(&mut self, pattern: &str) {
        self.excludes.push(pattern.to_string());
    }

    /// Number of include patterns.
    pub fn include_count(&self) -> usize {
        self.includes.len()
    }

    /// Number of exclude patterns.
    pub fn exclude_count(&self) -> usize {
        self.excludes.len()
    }

    /// Return `true` if `path` is included and not excluded.
    pub fn is_included(&self, path: &str) -> bool {
        let dominated_include = self.includes.iter().any(|p| Self::simple_match(p, path));
        if !dominated_include {
            return false;
        }
        let dominated_exclude = self.excludes.iter().any(|p| Self::simple_match(p, path));
        !dominated_exclude
    }

    /// Simple string-based matching.
    ///
    /// - `*.ext`  → suffix match on `.ext`
    /// - `dir/**` → prefix match on `dir/`
    /// - `**/x`   → suffix match on `/x` or exact match `x`
    /// - literal   → exact match or contained-in-path check
    fn simple_match(pattern: &str, path: &str) -> bool {
        if let Some(ext) = pattern.strip_prefix('*') {
            return path.ends_with(ext);
        }
        if let Some(prefix) = pattern.strip_suffix("/**") {
            return path.starts_with(prefix) && path.len() > prefix.len();
        }
        if let Some(suffix) = pattern.strip_prefix("**/") {
            return path == suffix || path.ends_with(&format!("/{suffix}"));
        }
        path == pattern || path.contains(pattern)
    }
}

impl Default for GlobNegation {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// GlobPriorityOrder
// ---------------------------------------------------------------------------

/// Stores glob patterns with an associated priority and can return them sorted
/// by priority (highest first).
#[derive(Debug, Clone)]
pub struct GlobPriorityOrder {
    entries: Vec<(String, u32)>,
}

impl GlobPriorityOrder {
    /// Create a new, empty priority list.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a pattern with the given priority.
    pub fn add_pattern(&mut self, pattern: &str, priority: u32) {
        self.entries.push((pattern.to_string(), priority));
    }

    /// Return patterns sorted by priority (highest first).
    pub fn sorted_patterns(&self) -> Vec<(&str, u32)> {
        let mut refs: Vec<(&str, u32)> = self
            .entries
            .iter()
            .map(|(p, pri)| (p.as_str(), *pri))
            .collect();
        refs.sort_by(|a, b| b.1.cmp(&a.1));
        refs
    }

    /// Return the pattern with the highest priority, or `None` if empty.
    pub fn highest_priority(&self) -> Option<(&str, u32)> {
        self.entries
            .iter()
            .max_by_key(|(_, pri)| *pri)
            .map(|(p, pri)| (p.as_str(), *pri))
    }

    /// Return the pattern with the lowest priority, or `None` if empty.
    pub fn lowest_priority(&self) -> Option<(&str, u32)> {
        self.entries
            .iter()
            .min_by_key(|(_, pri)| *pri)
            .map(|(p, pri)| (p.as_str(), *pri))
    }

    /// Return the number of stored patterns.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` if no patterns have been added.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for GlobPriorityOrder {
    fn default() -> Self {
        Self::new()
    }
}


// ── Glob Compilation Cache ──

use std::collections::HashMap as GlobCacheMap;

/// Caches compiled glob patterns to avoid redundant compilation.
#[derive(Debug)]
pub struct GlobCompilationCache {
    cache: GlobCacheMap<String, GlobPattern>,
    hits: usize,
    misses: usize,
    max_entries: usize,
}

impl GlobCompilationCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: GlobCacheMap::new(),
            hits: 0,
            misses: 0,
            max_entries: max_entries.max(1),
        }
    }

    /// Get or compile a glob pattern. Returns a reference to the cached pattern.
    pub fn get_or_compile(&mut self, pattern: &str) -> Result<&GlobPattern, GlobError> {
        if self.cache.contains_key(pattern) {
            self.hits += 1;
            return Ok(self.cache.get(pattern).unwrap());
        }
        self.misses += 1;
        let compiled = GlobPattern::new(pattern).map_err(GlobError::InvalidPattern)?;
        if self.cache.len() >= self.max_entries {
            // Evict the first entry (simple LRU approximation)
            if let Some(key) = self.cache.keys().next().cloned() {
                self.cache.remove(&key);
            }
        }
        self.cache.insert(pattern.to_string(), compiled);
        Ok(self.cache.get(pattern).unwrap())
    }

    pub fn contains(&self, pattern: &str) -> bool {
        self.cache.contains_key(pattern)
    }

    pub fn invalidate(&mut self, pattern: &str) -> bool {
        self.cache.remove(pattern).is_some()
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn size(&self) -> usize {
        self.cache.len()
    }

    pub fn hit_count(&self) -> usize {
        self.hits
    }

    pub fn miss_count(&self) -> usize {
        self.misses
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }
}

// ── Glob Match Statistics ──

/// Tracks match statistics for glob operations.
#[derive(Debug, Clone)]
pub struct GlobMatchStatistics {
    pattern_stats: GlobCacheMap<String, PatternStats>,
}

/// Statistics for a single glob pattern.
#[derive(Debug, Clone, Default)]
pub struct PatternStats {
    pub matches: usize,
    pub misses: usize,
    pub total_paths_tested: usize,
    pub last_matched_path: Option<String>,
}

impl PatternStats {
    pub fn match_rate(&self) -> f64 {
        if self.total_paths_tested == 0 {
            0.0
        } else {
            self.matches as f64 / self.total_paths_tested as f64
        }
    }
}

impl GlobMatchStatistics {
    pub fn new() -> Self {
        Self { pattern_stats: GlobCacheMap::new() }
    }

    pub fn record_match(&mut self, pattern: &str, path: &str) {
        let stats = self.pattern_stats.entry(pattern.to_string()).or_default();
        stats.matches += 1;
        stats.total_paths_tested += 1;
        stats.last_matched_path = Some(path.to_string());
    }

    pub fn record_miss(&mut self, pattern: &str) {
        let stats = self.pattern_stats.entry(pattern.to_string()).or_default();
        stats.misses += 1;
        stats.total_paths_tested += 1;
    }

    pub fn get_stats(&self, pattern: &str) -> Option<&PatternStats> {
        self.pattern_stats.get(pattern)
    }

    pub fn pattern_count(&self) -> usize {
        self.pattern_stats.len()
    }

    pub fn total_matches(&self) -> usize {
        self.pattern_stats.values().map(|s| s.matches).sum()
    }

    pub fn total_misses(&self) -> usize {
        self.pattern_stats.values().map(|s| s.misses).sum()
    }

    pub fn overall_match_rate(&self) -> f64 {
        let total = self.total_matches() + self.total_misses();
        if total == 0 { 0.0 } else { self.total_matches() as f64 / total as f64 }
    }

    pub fn top_patterns(&self, n: usize) -> Vec<(&str, &PatternStats)> {
        let mut entries: Vec<_> = self.pattern_stats.iter().map(|(k, v)| (k.as_str(), v)).collect();
        entries.sort_by(|a, b| b.1.matches.cmp(&a.1.matches));
        entries.truncate(n);
        entries
    }

    pub fn reset(&mut self) {
        self.pattern_stats.clear();
    }
}

impl Default for GlobMatchStatistics {
    fn default() -> Self { Self::new() }
}


// -- Glob Pattern Normalizer --

/// Normalizes glob patterns to a canonical form.
pub struct GlobPatternNormalizer;

impl GlobPatternNormalizer {
    /// Normalize path separators to forward slashes.
    pub fn normalize_separators(pattern: &str) -> String {
        pattern.replace('\\', "/")
    }

    /// Remove redundant segments like "./" and "foo/../".
    pub fn simplify(pattern: &str) -> String {
        let normalized = Self::normalize_separators(pattern);
        let mut parts: Vec<&str> = Vec::new();
        for segment in normalized.split('/') {
            match segment {
                "." | "" => {}
                ".." => { parts.pop(); }
                other => parts.push(other),
            }
        }
        if parts.is_empty() { ".".to_string() }
        else { parts.join("/") }
    }

    /// Check if a pattern is a simple (non-glob) path.
    pub fn is_simple_path(pattern: &str) -> bool {
        !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[')
    }

    /// Extract the non-glob prefix from a pattern (for optimization).
    pub fn extract_base_path(pattern: &str) -> &str {
        let glob_start = pattern.find(|c| c == '*' || c == '?' || c == '[')
            .unwrap_or(pattern.len());
        let last_sep = pattern[..glob_start].rfind('/').unwrap_or(0);
        if last_sep == 0 { "." } else { &pattern[..last_sep] }
    }
}


// ---------------------------------------------------------------------------
// vsedit-glob: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl GlobXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for GlobXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct GlobXRegistry {
    entries: Vec<GlobXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl GlobXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: GlobXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&GlobXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut GlobXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<GlobXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&GlobXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&GlobXConfig> {
        let mut sorted: Vec<&GlobXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&GlobXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> GlobXIterator<'_> {
        GlobXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct GlobXIterator<'a> {
    inner: std::slice::Iter<'a, GlobXConfig>,
}

impl<'a> Iterator for GlobXIterator<'a> {
    type Item = &'a GlobXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct GlobXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl GlobXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct GlobXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl GlobXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &GlobXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &GlobXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &GlobXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for GlobXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct GlobXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl GlobXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &GlobXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &GlobXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for GlobXValidator {
    fn default() -> Self {
        Self::new()
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
// xc_ pool and scheduler – generated block 86
// ---------------------------------------------------------------------------

/// Generic object pool `Xc86Pool<T>`.
pub struct Xc86Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc86Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc86PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc86Pool<T> {
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
    pub fn stats(&self) -> Xc86PoolStats {
        Xc86PoolStats {
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

impl<T> Default for Xc86Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc86Scheduler`.
pub struct Xc86Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc86Scheduler {
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

impl Default for Xc86Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_86 hash for the given byte slice.
pub fn xc_86_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_86 convention.
pub fn xc_86_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_13 deepening: state machine + event bus ---

/// States for the Xd13 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd13State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd13State {
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
pub struct Xd13Transition {
    pub from: Xd13State,
    pub to: Xd13State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd13StateMachine {
    current: Xd13State,
    history: Vec<Xd13Transition>,
    step_counter: usize,
}

impl Xd13StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd13State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd13State {
        self.current
    }

    pub fn history(&self) -> &[Xd13Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd13State) -> Result<Xd13State, String> {
        let allowed = match (self.current, target) {
            (Xd13State::Idle, Xd13State::Running) => true,
            (Xd13State::Running, Xd13State::Paused) => true,
            (Xd13State::Running, Xd13State::Done) => true,
            (Xd13State::Paused, Xd13State::Running) => true,
            (Xd13State::Paused, Xd13State::Done) => true,
            (Xd13State::Done, Xd13State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_13: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd13Transition {
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
            "Xd13SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd13State> {
        let prefix = "Xd13SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd13State::Idle),
            "Running" => Some(Xd13State::Running),
            "Paused" => Some(Xd13State::Paused),
            "Done" => Some(Xd13State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd13State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd13 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd13Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd13Event {
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

type Xd13HandlerFn = Box<dyn Fn(&Xd13Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd13EventBus {
    handlers: Vec<(usize, Option<String>, Xd13HandlerFn)>,
    next_id: usize,
    published: Vec<Xd13Event>,
}

impl Xd13EventBus {
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
        F: Fn(&Xd13Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd13Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd13Event) {
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

    pub fn published_events(&self) -> &[Xd13Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #11
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf11Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf11TrieNode {
    children: std::collections::HashMap<char, Xf11TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf11Trie {
    root: Xf11TrieNode,
    count: usize,
}

impl Xf11Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf11TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf11TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf11TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf11BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf11BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 85).
pub struct Xh85SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh85SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 127 as u64,
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

/// A compact bit set supporting boolean operations (variant 85).
pub struct Xh85BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh85BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 85).
pub struct Xi85Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi85Deque<T> {
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
pub struct Xi85Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi85Interval {
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

/// A simple interval tree (variant 85).
pub struct Xi85IntervalTree {
    xi_intervals: Vec<Xi85Interval>,
}

impl Xi85IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi85Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi85Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi85Interval) -> Vec<&Xi85Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi85Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi85Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi85Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi85Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi85Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi85Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 85) ---

/// Disjoint set / union-find for crate 85.
pub struct Xj85UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj85UnionFind {
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

const XJ85_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 85.
pub struct Xj85BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj85BTreeNode<K, V>>>,
    len: usize,
}

struct Xj85BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj85BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj85BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ85_BTREE_ORDER - 1
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
        let mid = XJ85_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj85BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj85BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj85BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj85BTreeNode::xj_new_leaf();
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


// --- xk_85 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk85SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk85SegmentTree {
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
pub struct Xk85DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk85DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_85).
#[derive(Debug, Clone)]
pub struct Xl85Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl85Rope {
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

/// Suffix array for efficient string searching (xl_85).
#[derive(Debug, Clone)]
pub struct Xl85SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl85SuffixArray {
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
pub struct Xm85MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm85MatrixSparse {
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
pub struct Xm85Tokenizer {
    text: String,
}

impl Xm85Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 85.
pub struct Xn85Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn85Fenwick {
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

// ----- AVL tree map — crate 85 -----

#[derive(Debug, Clone)]
struct Xn85AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn85AvlNode<K, V>>>,
    right: Option<Box<Xn85AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 85.
#[derive(Debug, Clone)]
pub struct Xn85AVL<K, V> {
    root: Option<Box<Xn85AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn85AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn85AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn85AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn85AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn85AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn85AvlNode<K, V>>) -> Box<Xn85AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn85AvlNode<K, V>>) -> Box<Xn85AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn85AvlNode<K, V>>) -> Box<Xn85AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn85AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn85AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn85AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn85AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn85AvlNode<K, V>>) -> &Xn85AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn85AvlNode<K, V>>) -> (Box<Xn85AvlNode<K, V>>, Option<Box<Xn85AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn85AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn85AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn85AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn85AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn85AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn85AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn85AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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


// ---------------------------------------------------------------------------
// Xo85RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo85Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo85RBNode<K, V> {
    key: K,
    value: V,
    color: Xo85Color,
    left: Option<Box<Xo85RBNode<K, V>>>,
    right: Option<Box<Xo85RBNode<K, V>>>,
}

/// A red-black tree map for crate 85.
#[derive(Debug, Clone)]
pub struct Xo85RedBlack<K, V> {
    root: Option<Box<Xo85RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo85RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo85Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo85RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo85RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo85RBNode {
                    key, value, color: Xo85Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo85RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo85Color::Red)
    }

    fn xo_balance(mut h: Box<Xo85RBNode<K, V>>) -> Box<Xo85RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo85Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo85RBNode<K, V>>) -> Box<Xo85RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo85Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo85RBNode<K, V>>) -> Box<Xo85RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo85Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo85RBNode<K, V>>) {
        h.color = Xo85Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo85Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo85Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo85Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo85RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo85RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo85RBNode<K, V>) -> (K, V, Option<Box<Xo85RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo85RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo85Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo85RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo85ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 85.
#[derive(Debug, Clone)]
pub struct Xo85ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo85ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo85#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo85#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 85).
#[derive(Debug)]
pub struct Xp85SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp85Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp85Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp85Node<K, V>>>,
    xp_right: Option<Box<Xp85Node<K, V>>>,
}

impl<K: Ord, V> Xp85Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp85SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp85SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp85Node<K, V>>>, key: &K) -> Option<Box<Xp85Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp85Node<K, V>>) -> Box<Xp85Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp85Node<K, V>>) -> Box<Xp85Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp85Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp85Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp85Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq85Treap ---------------

use std::cmp::Ordering as Xq85Ord;

struct Xq85TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq85TreapNode<K, V>>>,
    right: Option<Box<Xq85TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq85Treap<K, V> {
    root: Option<Box<Xq85TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq85TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_85_size<K, V>(node: &Option<Box<Xq85TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_85_update_size<K, V>(node: &mut Xq85TreapNode<K, V>) {
    node.size = 1 + xq_85_size(&node.left) + xq_85_size(&node.right);
}

fn xq_85_rotate_right<K, V>(mut node: Box<Xq85TreapNode<K, V>>) -> Box<Xq85TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_85_update_size(&mut node);
    left.right = Some(node);
    xq_85_update_size(&mut left);
    left
}

fn xq_85_rotate_left<K, V>(mut node: Box<Xq85TreapNode<K, V>>) -> Box<Xq85TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_85_update_size(&mut node);
    right.left = Some(node);
    xq_85_update_size(&mut right);
    right
}

fn xq_85_insert_node<K: Ord, V>(
    node: Option<Box<Xq85TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq85TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq85TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq85Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq85Ord::Less => {
                let (new_left, old) = xq_85_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_85_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_85_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq85Ord::Greater => {
                let (new_right, old) = xq_85_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_85_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_85_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_85_remove_node<K: Ord, V>(
    node: Option<Box<Xq85TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq85TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq85Ord::Less => {
                let (new_left, old) = xq_85_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_85_update_size(&mut n);
                (Some(n), old)
            }
            Xq85Ord::Greater => {
                let (new_right, old) = xq_85_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_85_update_size(&mut n);
                (Some(n), old)
            }
            Xq85Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_85_rotate_right(n);
                    let (new_right, old) = xq_85_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_85_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_85_rotate_left(n);
                    let (new_left, old) = xq_85_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_85_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_85_find_min<K, V>(node: &Option<Box<Xq85TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_85_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_85_find_max<K, V>(node: &Option<Box<Xq85TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_85_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_85_rank<K: Ord, V>(node: &Option<Box<Xq85TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq85Ord::Less => xq_85_rank(&n.left, key),
            Xq85Ord::Equal => xq_85_size(&n.left),
            Xq85Ord::Greater => 1 + xq_85_size(&n.left) + xq_85_rank(&n.right, key),
        },
    }
}

fn xq_85_kth<K, V>(node: &Option<Box<Xq85TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_85_size(&n.left);
        if k < left_size {
            xq_85_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_85_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_85_in_order<K: Clone, V>(node: &Option<Box<Xq85TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_85_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_85_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq85Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 85 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_85_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq85Ord::Equal => return Some(&n.value),
                Xq85Ord::Less => cur = &n.left,
                Xq85Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_85_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_85_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_85_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_85_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_85_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_85_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_85_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq85VEBTree ---------------

pub struct Xq85VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq85VEBTree>>,
    clusters: Vec<Option<Box<Xq85VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq85VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq85VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq85VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr85KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr85KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr85BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr85KDNode {
    xr_point: Xr85KDPoint,
    xr_left: Option<Box<Xr85KDNode>>,
    xr_right: Option<Box<Xr85KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr85KDTree {
    xr_root: Option<Box<Xr85KDNode>>,
    xr_size: usize,
}

impl Xr85KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr85KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr85KDNode>>,
        point: Xr85KDPoint,
        depth: usize,
    ) -> Box<Xr85KDNode> {
        match node {
            None => Box::new(Xr85KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr85KDPoint) -> Option<Xr85KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr85KDNode>,
        query: &Xr85KDPoint,
        depth: usize,
        best: &mut Xr85KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr85KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr85KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr85KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr85KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr85KDNode>>, pts: &mut Vec<Xr85KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr85KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr85BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr85BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs85PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs85PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs85PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs85PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs85ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs85ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs85ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs85RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs85RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs85RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs85CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs85CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs85CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}

/// Auxiliary statistics tracker for xs_85 data structures.
#[derive(Debug, Clone)]
pub struct Xs85StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs85StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs85StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
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

    #[test]
    fn classify_glob_literal() {
        assert_eq!(classify_glob("src/main.rs"), GlobComplexity::Literal);
    }

    #[test]
    fn classify_glob_simple_star() {
        assert_eq!(classify_glob("*.rs"), GlobComplexity::Simple);
        assert_eq!(classify_glob("src/?.rs"), GlobComplexity::Simple);
    }

    #[test]
    fn classify_glob_character_class() {
        assert_eq!(classify_glob("[abc].txt"), GlobComplexity::CharacterClass);
    }

    #[test]
    fn classify_glob_alternation() {
        assert_eq!(classify_glob("*.{rs,toml}"), GlobComplexity::Alternation);
    }

    #[test]
    fn classify_glob_recursive() {
        assert_eq!(classify_glob("**/*.rs"), GlobComplexity::Recursive);
    }

    #[test]
    fn extract_extension_rs() {
        assert_eq!(extract_extension_from_glob("*.rs"), Some("rs"));
        assert_eq!(extract_extension_from_glob("**/*.toml"), Some("toml"));
    }

    #[test]
    fn extract_extension_no_match() {
        assert_eq!(extract_extension_from_glob("src/main.rs"), None);
        assert_eq!(extract_extension_from_glob("*"), None);
    }

    #[test]
    fn common_glob_prefix_shared() {
        let patterns = &["src/**/*.rs", "src/**/*.toml", "src/lib.rs"];
        assert_eq!(common_glob_prefix(patterns), "src/");
    }

    #[test]
    fn common_glob_prefix_none() {
        let patterns = &["*.rs", "tests/*.rs"];
        assert_eq!(common_glob_prefix(patterns), "");
    }

    #[test]
    fn common_glob_prefix_empty_input() {
        let patterns: &[&str] = &[];
        assert_eq!(common_glob_prefix(patterns), "");
    }

    #[test]
    fn pattern_set_matching_subset() {
        let set = GlobPatternSet::new(&["*.rs", "*.toml", "*.md"]).unwrap();
        let sub = set.matching_subset("Cargo.toml").unwrap();
        assert_eq!(sub.pattern_count(), 1);
        assert!(sub.contains_pattern("*.toml"));
    }

    #[test]
    fn pattern_set_complexities() {
        let set = GlobPatternSet::new(&["*.rs", "**/*.toml", "src/main.rs"]).unwrap();
        let cx = set.complexities();
        assert_eq!(cx, vec![GlobComplexity::Simple, GlobComplexity::Recursive, GlobComplexity::Literal]);
    }

    #[test]
    fn file_filter_pattern_counts() {
        let f = FileFilter::new(&["*.rs"], &["*.bak", "*.tmp"]).unwrap();
        assert_eq!(f.pattern_counts(), (1, 2));
    }

    #[test]
    fn file_filter_passthrough() {
        let f = FileFilter::new(&[], &[]).unwrap();
        assert!(f.is_passthrough());
        let f2 = FileFilter::new(&["*.rs"], &[]).unwrap();
        assert!(!f2.is_passthrough());
    }

    #[test]
    fn file_filter_builder_counts() {
        let b = FileFilterBuilder::new().include("*.rs").exclude("*.bak");
        assert_eq!(b.include_count(), 1);
        assert_eq!(b.exclude_count(), 1);
        assert!(!b.is_empty());
    }

    #[test]
    fn file_filter_builder_empty() {
        let b = FileFilterBuilder::new();
        assert!(b.is_empty());
        assert_eq!(b.include_count(), 0);
    }

    // -----------------------------------------------------------------------
    // GlobOptimizer tests
    // -----------------------------------------------------------------------

    #[test]
    fn optimizer_removes_literal_covered_by_wildcard() {
        let mut opt = GlobOptimizer::new();
        opt.add_pattern("*.rs");
        opt.add_pattern("main.rs");
        assert!(opt.has_redundant());
        let result = opt.optimize();
        assert_eq!(result, vec!["*.rs"]);
    }

    #[test]
    fn optimizer_keeps_unrelated_patterns() {
        let mut opt = GlobOptimizer::new();
        opt.add_pattern("*.rs");
        opt.add_pattern("*.toml");
        assert!(!opt.has_redundant());
        assert_eq!(opt.optimize().len(), 2);
    }

    #[test]
    fn optimizer_doublestar_covers_everything() {
        let mut opt = GlobOptimizer::new();
        opt.add_pattern("**");
        opt.add_pattern("src/main.rs");
        opt.add_pattern("*.toml");
        assert_eq!(opt.optimize(), vec!["**"]);
    }

    #[test]
    fn optimizer_pattern_count() {
        let mut opt = GlobOptimizer::new();
        assert_eq!(opt.pattern_count(), 0);
        opt.add_pattern("*.rs");
        opt.add_pattern("*.ts");
        assert_eq!(opt.pattern_count(), 2);
    }

    // -----------------------------------------------------------------------
    // GlobExplainer tests
    // -----------------------------------------------------------------------

    #[test]
    fn explainer_star_ext() {
        let desc = GlobExplainer::explain("*.rs");
        assert!(desc.contains(".rs"), "got: {desc}");
    }

    #[test]
    fn explainer_doublestar_ext() {
        let desc = GlobExplainer::explain("**/*.txt");
        assert!(desc.contains(".txt"), "got: {desc}");
        assert!(desc.contains("subdirectory"), "got: {desc}");
    }

    #[test]
    fn explainer_dir_star() {
        let desc = GlobExplainer::explain("src/**");
        assert!(desc.contains("src/"), "got: {desc}");
    }

    #[test]
    fn complexity_simple() {
        assert_eq!(GlobExplainer::complexity("*.rs"), ExplainerComplexity::Simple);
    }

    #[test]
    fn complexity_moderate() {
        assert_eq!(GlobExplainer::complexity("**/*.rs"), ExplainerComplexity::Complex);
    }

    #[test]
    fn complexity_display() {
        assert_eq!(format!("{}", ExplainerComplexity::Simple), "Simple");
        assert_eq!(format!("{}", ExplainerComplexity::Complex), "Complex");
    }

    // -----------------------------------------------------------------------
    // GlobNegation tests
    // -----------------------------------------------------------------------

    #[test]
    fn negation_include_and_exclude() {
        let mut neg = GlobNegation::new();
        neg.add_include("*.rs");
        neg.add_exclude("*.bak");
        assert!(neg.is_included("main.rs"));
        assert!(!neg.is_included("backup.bak"));
        assert!(!neg.is_included("readme.md"));
    }

    #[test]
    fn negation_counts() {
        let mut neg = GlobNegation::new();
        neg.add_include("*.rs");
        neg.add_include("*.toml");
        neg.add_exclude("*.bak");
        assert_eq!(neg.include_count(), 2);
        assert_eq!(neg.exclude_count(), 1);
    }

    #[test]
    fn negation_exclude_overrides_include() {
        let mut neg = GlobNegation::new();
        neg.add_include("*.rs");
        neg.add_exclude("*.rs");
        assert!(!neg.is_included("main.rs"));
    }

    // -----------------------------------------------------------------------
    // GlobPriorityOrder tests
    // -----------------------------------------------------------------------

    #[test]
    fn priority_sorted() {
        let mut prio = GlobPriorityOrder::new();
        prio.add_pattern("*.rs", 10);
        prio.add_pattern("*.toml", 50);
        prio.add_pattern("*.lock", 1);
        let sorted = prio.sorted_patterns();
        assert_eq!(sorted[0], ("*.toml", 50));
        assert_eq!(sorted[2], ("*.lock", 1));
    }

    #[test]
    fn priority_highest_lowest() {
        let mut prio = GlobPriorityOrder::new();
        prio.add_pattern("a", 5);
        prio.add_pattern("b", 100);
        prio.add_pattern("c", 1);
        assert_eq!(prio.highest_priority(), Some(("b", 100)));
        assert_eq!(prio.lowest_priority(), Some(("c", 1)));
    }

    #[test]
    fn priority_empty() {
        let prio = GlobPriorityOrder::new();
        assert!(prio.is_empty());
        assert_eq!(prio.len(), 0);
        assert_eq!(prio.highest_priority(), None);
    }

    // ── Glob Compilation Cache Tests ──

    #[test]
    fn test_cache_compile_and_hit() {
        let mut cache = GlobCompilationCache::new(10);
        assert!(cache.get_or_compile("*.rs").is_ok());
        assert!(cache.get_or_compile("*.rs").is_ok());
        assert_eq!(cache.hit_count(), 1);
        assert_eq!(cache.miss_count(), 1);
    }

    #[test]
    fn test_cache_invalid_pattern() {
        let mut cache = GlobCompilationCache::new(10);
        assert!(cache.get_or_compile("[invalid").is_err());
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = GlobCompilationCache::new(2);
        cache.get_or_compile("*.rs").ok();
        cache.get_or_compile("*.toml").ok();
        cache.get_or_compile("*.md").ok();
        assert_eq!(cache.size(), 2);
    }

    #[test]
    fn test_cache_invalidate() {
        let mut cache = GlobCompilationCache::new(10);
        cache.get_or_compile("*.rs").ok();
        assert!(cache.invalidate("*.rs"));
        assert!(!cache.contains("*.rs"));
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = GlobCompilationCache::new(10);
        cache.get_or_compile("*.rs").ok();
        cache.clear();
        assert_eq!(cache.size(), 0);
        assert_eq!(cache.hit_count(), 0);
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut cache = GlobCompilationCache::new(10);
        cache.get_or_compile("*.rs").ok();
        cache.get_or_compile("*.rs").ok();
        cache.get_or_compile("*.rs").ok();
        assert!((cache.hit_rate() - 2.0 / 3.0).abs() < 1e-9);
    }

    // ── Glob Match Statistics Tests ──

    #[test]
    fn test_stats_record_match() {
        let mut stats = GlobMatchStatistics::new();
        stats.record_match("*.rs", "main.rs");
        assert_eq!(stats.total_matches(), 1);
        assert_eq!(stats.get_stats("*.rs").unwrap().last_matched_path, Some("main.rs".into()));
    }

    #[test]
    fn test_stats_record_miss() {
        let mut stats = GlobMatchStatistics::new();
        stats.record_miss("*.rs");
        assert_eq!(stats.total_misses(), 1);
    }

    #[test]
    fn test_stats_match_rate() {
        let mut stats = GlobMatchStatistics::new();
        stats.record_match("*.rs", "a.rs");
        stats.record_miss("*.rs");
        let ps = stats.get_stats("*.rs").unwrap();
        assert!((ps.match_rate() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_stats_top_patterns() {
        let mut stats = GlobMatchStatistics::new();
        stats.record_match("*.rs", "a.rs");
        stats.record_match("*.rs", "b.rs");
        stats.record_match("*.toml", "Cargo.toml");
        let top = stats.top_patterns(1);
        assert_eq!(top[0].0, "*.rs");
    }

    #[test]
    fn test_stats_reset() {
        let mut stats = GlobMatchStatistics::new();
        stats.record_match("*.rs", "a.rs");
        stats.reset();
        assert_eq!(stats.pattern_count(), 0);
    }

    #[test]
    fn test_stats_overall_rate() {
        let mut stats = GlobMatchStatistics::new();
        stats.record_match("*.rs", "a.rs");
        stats.record_match("*.rs", "b.rs");
        stats.record_miss("*.toml");
        assert!((stats.overall_match_rate() - 2.0 / 3.0).abs() < 1e-9);
    }


    // -- Glob Normalizer Tests --

    #[test]
    fn test_normalizer_separators() {
        assert_eq!(GlobPatternNormalizer::normalize_separators("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn test_normalizer_simplify() {
        assert_eq!(GlobPatternNormalizer::simplify("./src/../src/main.rs"), "src/main.rs");
    }

    #[test]
    fn test_normalizer_is_simple() {
        assert!(GlobPatternNormalizer::is_simple_path("src/main.rs"));
        assert!(!GlobPatternNormalizer::is_simple_path("*.rs"));
    }

    #[test]
    fn test_normalizer_base_path() {
        assert_eq!(GlobPatternNormalizer::extract_base_path("src/test/*.rs"), "src/test");
        assert_eq!(GlobPatternNormalizer::extract_base_path("*.rs"), ".");
    }


    #[test]
    fn glob_x_config_new() {
        let c = GlobXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn glob_x_config_builder() {
        let c = GlobXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn glob_x_config_display() {
        let c = GlobXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn glob_x_registry_insert_get() {
        let mut reg = GlobXRegistry::new();
        reg.insert(GlobXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn glob_x_registry_duplicate() {
        let mut reg = GlobXRegistry::new();
        reg.insert(GlobXConfig::new("a")).unwrap();
        assert!(reg.insert(GlobXConfig::new("a")).is_err());
    }

    #[test]
    fn glob_x_registry_remove() {
        let mut reg = GlobXRegistry::new();
        reg.insert(GlobXConfig::new("a")).unwrap();
        reg.insert(GlobXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn glob_x_registry_active_entries() {
        let mut reg = GlobXRegistry::new();
        reg.insert(GlobXConfig::new("a")).unwrap();
        reg.insert(GlobXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn glob_x_registry_by_weight() {
        let mut reg = GlobXRegistry::new();
        reg.insert(GlobXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(GlobXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn glob_x_registry_tags() {
        let mut reg = GlobXRegistry::new();
        reg.insert(GlobXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(GlobXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn glob_x_registry_total_weight() {
        let mut reg = GlobXRegistry::new();
        reg.insert(GlobXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(GlobXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn glob_x_registry_iterator() {
        let mut reg = GlobXRegistry::new();
        reg.insert(GlobXConfig::new("a")).unwrap();
        reg.insert(GlobXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn glob_x_cache_put_get() {
        let mut cache = GlobXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn glob_x_cache_eviction() {
        let mut cache = GlobXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn glob_x_cache_lru_order() {
        let mut cache = GlobXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn glob_x_cache_most_least_recent() {
        let mut cache = GlobXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn glob_x_formatter_entry() {
        let e = GlobXConfig::new("k").with_value("v");
        let fmt = GlobXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn glob_x_formatter_summary() {
        let mut reg = GlobXRegistry::new();
        reg.insert(GlobXConfig::new("a").with_weight(5)).unwrap();
        let fmt = GlobXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn glob_x_validator_valid() {
        let v = GlobXValidator::new();
        let c = GlobXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn glob_x_validator_empty_key() {
        let v = GlobXValidator::new();
        let c = GlobXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn glob_x_validator_require_value() {
        let v = GlobXValidator::new().require_value(true);
        let c = GlobXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn glob_x_validator_allowed_tags() {
        let v = GlobXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = GlobXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn glob_x_validator_validate_all() {
        let v = GlobXValidator::new();
        let mut reg = GlobXRegistry::new();
        reg.insert(GlobXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
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


    // ---- xc_ pool / scheduler tests – block 86 ----

    #[test]
    fn xc_86_pool_new_empty() {
        let pool: super::Xc86Pool<i32> = super::Xc86Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_86_pool_release_acquire() {
        let mut pool = super::Xc86Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_86_pool_acquire_empty() {
        let mut pool: super::Xc86Pool<i32> = super::Xc86Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_86_pool_full() {
        let mut pool = super::Xc86Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_86_pool_drain() {
        let mut pool = super::Xc86Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_86_pool_stats() {
        let mut pool = super::Xc86Pool::new(8);
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
    fn xc_86_pool_clear() {
        let mut pool = super::Xc86Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_86_pool_shrink() {
        let mut pool = super::Xc86Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_86_pool_default() {
        let pool: super::Xc86Pool<String> = super::Xc86Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_86_pool_extend() {
        let mut pool = super::Xc86Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_86_pool_retain() {
        let mut pool = super::Xc86Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_86_scheduler_round_robin() {
        let mut sched = super::Xc86Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_86_scheduler_empty() {
        let mut sched = super::Xc86Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_86_scheduler_reset() {
        let mut sched = super::Xc86Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_86_scheduler_add_remove() {
        let mut sched = super::Xc86Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_86_scheduler_targets() {
        let sched = super::Xc86Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_86_hash_empty() {
        assert_eq!(super::xc_86_hash(b""), 5381);
    }

    #[test]
    fn xc_86_hash_data() {
        let h = super::xc_86_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_86_hash(b"hello"), h);
    }

    #[test]
    fn xc_86_reverse_str() {
        assert_eq!(super::xc_86_reverse("abc"), "cba");
        assert_eq!(super::xc_86_reverse(""), "");
    }


    // --- xd_13 deepening tests ---

    #[test]
    fn xd_13_sm_initial_state() {
        let sm = Xd13StateMachine::new();
        assert_eq!(sm.current_state(), Xd13State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_13_sm_valid_idle_to_running() {
        let mut sm = Xd13StateMachine::new();
        assert!(sm.transition(Xd13State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd13State::Running);
    }

    #[test]
    fn xd_13_sm_valid_running_to_paused() {
        let mut sm = Xd13StateMachine::new();
        sm.transition(Xd13State::Running).unwrap();
        assert!(sm.transition(Xd13State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd13State::Paused);
    }

    #[test]
    fn xd_13_sm_valid_running_to_done() {
        let mut sm = Xd13StateMachine::new();
        sm.transition(Xd13State::Running).unwrap();
        assert!(sm.transition(Xd13State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd13State::Done);
    }

    #[test]
    fn xd_13_sm_valid_paused_to_running() {
        let mut sm = Xd13StateMachine::new();
        sm.transition(Xd13State::Running).unwrap();
        sm.transition(Xd13State::Paused).unwrap();
        assert!(sm.transition(Xd13State::Running).is_ok());
    }

    #[test]
    fn xd_13_sm_valid_done_to_idle() {
        let mut sm = Xd13StateMachine::new();
        sm.transition(Xd13State::Running).unwrap();
        sm.transition(Xd13State::Done).unwrap();
        assert!(sm.transition(Xd13State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd13State::Idle);
    }

    #[test]
    fn xd_13_sm_invalid_idle_to_done() {
        let mut sm = Xd13StateMachine::new();
        assert!(sm.transition(Xd13State::Done).is_err());
    }

    #[test]
    fn xd_13_sm_invalid_idle_to_paused() {
        let mut sm = Xd13StateMachine::new();
        assert!(sm.transition(Xd13State::Paused).is_err());
    }

    #[test]
    fn xd_13_sm_history_tracking() {
        let mut sm = Xd13StateMachine::new();
        sm.transition(Xd13State::Running).unwrap();
        sm.transition(Xd13State::Paused).unwrap();
        sm.transition(Xd13State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd13State::Idle);
        assert_eq!(sm.history()[0].to, Xd13State::Running);
        assert_eq!(sm.history()[1].from, Xd13State::Running);
        assert_eq!(sm.history()[2].to, Xd13State::Done);
    }

    #[test]
    fn xd_13_sm_serialize_deserialize() {
        let mut sm = Xd13StateMachine::new();
        sm.transition(Xd13State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd13StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd13State::Running));
    }

    #[test]
    fn xd_13_sm_deserialize_invalid() {
        assert_eq!(Xd13StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_13_sm_reset() {
        let mut sm = Xd13StateMachine::new();
        sm.transition(Xd13State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd13State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_13_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd13EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd13Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_13_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd13EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd13Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd13Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_13_bus_unsubscribe() {
        let mut bus = Xd13EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_13_event_kind_and_payload() {
        let e = Xd13Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd13Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_13_bus_clear_history() {
        let mut bus = Xd13EventBus::new();
        bus.publish(Xd13Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_13_sm_step_counter_increments() {
        let mut sm = Xd13StateMachine::new();
        sm.transition(Xd13State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd13State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #11 --

    #[test]
    fn xf11_trie_insert_search() {
        let mut t = Xf11Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf11_trie_starts_with() {
        let mut t = Xf11Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf11_trie_remove() {
        let mut t = Xf11Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf11_trie_word_count() {
        let mut t = Xf11Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf11_trie_longest_prefix() {
        let mut t = Xf11Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf11_trie_all_words() {
        let mut t = Xf11Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf11_trie_autocomplete() {
        let mut t = Xf11Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf11_trie_empty_search() {
        let t = Xf11Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf11_bloom_add_contains() {
        let mut bf = Xf11BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf11_bloom_probably_absent() {
        let bf = Xf11BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf11_bloom_false_positive_rate() {
        let mut bf = Xf11BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf11_bloom_clear() {
        let mut bf = Xf11BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf11_bloom_union() {
        let mut a = Xf11BloomFilter::xf_new(512, 2);
        let mut b = Xf11BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf11_bloom_intersection_estimate() {
        let mut a = Xf11BloomFilter::xf_new(512, 2);
        let mut b = Xf11BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf11_bloom_union_size_mismatch() {
        let a = Xf11BloomFilter::xf_new(256, 2);
        let b = Xf11BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh85_skip_insert_contains() {
        let mut sl = super::Xh85SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh85_skip_remove() {
        let mut sl = super::Xh85SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh85_skip_len() {
        let mut sl = super::Xh85SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh85_skip_range_query() {
        let mut sl = super::Xh85SkipList::xh_new(4);
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
    fn xh85_skip_floor_ceiling() {
        let mut sl = super::Xh85SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh85_skip_rank() {
        let mut sl = super::Xh85SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh85_skip_empty() {
        let sl = super::Xh85SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh85_skip_duplicates() {
        let mut sl = super::Xh85SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh85_bitset_set_test() {
        let mut bs = super::Xh85BitSet::xh_new(256);
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
    fn xh85_bitset_clear_count() {
        let mut bs = super::Xh85BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh85_bitset_and_or_xor() {
        let mut a = super::Xh85BitSet::xh_new(128);
        let mut b = super::Xh85BitSet::xh_new(128);
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
    fn xh85_bitset_iter_ones() {
        let mut bs = super::Xh85BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh85_bitset_first_last() {
        let mut bs = super::Xh85BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh85_bitset_empty() {
        let bs = super::Xh85BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi85_deque_push_pop_back() {
        let mut dq = super::Xi85Deque::xi_new(4);
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
    fn xi85_deque_push_pop_front() {
        let mut dq = super::Xi85Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi85_deque_mixed_ops() {
        let mut dq = super::Xi85Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi85_deque_get_and_split() {
        let mut dq = super::Xi85Deque::xi_new(8);
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
    fn xi85_deque_rotate_left() {
        let mut dq = super::Xi85Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi85_deque_rotate_right() {
        let mut dq = super::Xi85Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi85_deque_grow() {
        let mut dq = super::Xi85Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi85_deque_empty() {
        let dq = super::Xi85Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi85_interval_tree_insert_query() {
        let mut tree = super::Xi85IntervalTree::xi_new();
        tree.xi_insert(super::Xi85Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi85Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi85Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi85_interval_tree_overlap() {
        let mut tree = super::Xi85IntervalTree::xi_new();
        tree.xi_insert(super::Xi85Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi85Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi85Interval::xi_new(12, 20));
        let q = super::Xi85Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi85_interval_tree_remove() {
        let mut tree = super::Xi85IntervalTree::xi_new();
        tree.xi_insert(super::Xi85Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi85Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi85_interval_tree_gaps() {
        let mut tree = super::Xi85IntervalTree::xi_new();
        tree.xi_insert(super::Xi85Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi85Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi85Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi85Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi85Interval::xi_new(8, 10));
    }

    #[test]
    fn xi85_interval_tree_merge() {
        let mut tree = super::Xi85IntervalTree::xi_new();
        tree.xi_insert(super::Xi85Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi85Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi85Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi85Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi85Interval::xi_new(10, 15));
    }

    #[test]
    fn xi85_interval_tree_all() {
        let mut tree = super::Xi85IntervalTree::xi_new();
        tree.xi_insert(super::Xi85Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi85Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi85_interval_tree_empty() {
        let tree = super::Xi85IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi85_interval_tree_contains_point() {
        let iv = super::Xi85Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 85) ---

    #[test]
    fn xj_85_uf_make_and_find() {
        let mut uf = super::Xj85UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_85_uf_union_connected() {
        let mut uf = super::Xj85UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_85_uf_component_count() {
        let mut uf = super::Xj85UnionFind::xj_new();
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
    fn xj_85_uf_component_size() {
        let mut uf = super::Xj85UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_85_uf_largest_component() {
        let mut uf = super::Xj85UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_85_uf_many_elements() {
        let mut uf = super::Xj85UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_85_uf_separate_components() {
        let mut uf = super::Xj85UnionFind::xj_new();
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
    fn xj_85_uf_path_compression() {
        let mut uf = super::Xj85UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_85_bt_insert_get() {
        let mut bt = super::Xj85BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_85_bt_contains_len() {
        let mut bt = super::Xj85BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_85_bt_replace() {
        let mut bt = super::Xj85BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_85_bt_remove() {
        let mut bt = super::Xj85BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_85_bt_keys_values() {
        let mut bt = super::Xj85BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_85_bt_range() {
        let mut bt = super::Xj85BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_85_bt_min_max() {
        let mut bt = super::Xj85BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_85_bt_many_inserts() {
        let mut bt = super::Xj85BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_85 segment tree tests ---

    #[test]
    fn xk_85_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk85SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_85_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk85SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_85_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk85SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_85_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk85SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_85_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk85SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_85_st_single_element() {
        let data = vec![42];
        let st = super::Xk85SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_85_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk85SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_85_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk85SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_85 disjoint intervals tests ---

    #[test]
    fn xk_85_di_add_and_count() {
        let mut di = super::Xk85DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_85_di_merge_overlap() {
        let mut di = super::Xk85DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_85_di_contains() {
        let mut di = super::Xk85DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_85_di_remove() {
        let mut di = super::Xk85DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_85_di_covered_length() {
        let mut di = super::Xk85DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_85_di_gaps() {
        let mut di = super::Xk85DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_85_di_merge_adjacent() {
        let mut di = super::Xk85DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_85_di_empty() {
        let di = super::Xk85DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_85_rope_new_empty() {
        let rope = super::Xl85Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_85_rope_from_str() {
        let rope = super::Xl85Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_85_rope_insert_at() {
        let mut rope = super::Xl85Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_85_rope_delete_range() {
        let mut rope = super::Xl85Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_85_rope_char_at() {
        let rope = super::Xl85Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_85_rope_split_concat() {
        let rope = super::Xl85Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_85_rope_line_count() {
        let rope = super::Xl85Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_85_rope_line_at() {
        let rope = super::Xl85Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_85_sa_build_and_search() {
        let sa = super::Xl85SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_85_sa_count() {
        let sa = super::Xl85SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_85_sa_longest_repeated() {
        let sa = super::Xl85SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_85_sa_all_positions() {
        let sa = super::Xl85SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_85_sa_len() {
        let sa = super::Xl85SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_85_sa_empty() {
        let sa = super::Xl85SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_85_rope_slice() {
        let rope = super::Xl85Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_85_sa_search_start() {
        let sa = super::Xl85SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_85_sparse_set_get() {
        let mut m = super::Xm85MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_85_sparse_row_col() {
        let mut m = super::Xm85MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_85_sparse_transpose() {
        let mut m = super::Xm85MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_85_sparse_multiply_vec() {
        let mut m = super::Xm85MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_85_sparse_nnz_density() {
        let mut m = super::Xm85MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_85_sparse_clear() {
        let mut m = super::Xm85MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_85_sparse_overwrite_zero() {
        let mut m = super::Xm85MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_85_tokenizer_basic() {
        let t = super::Xm85Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_85_tokenizer_count() {
        let t = super::Xm85Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_85_tokenizer_unique() {
        let t = super::Xm85Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_85_tokenizer_frequency() {
        let t = super::Xm85Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_85_tokenizer_delimiter() {
        let t = super::Xm85Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_85_tokenizer_whitespace() {
        let t = super::Xm85Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_85_tokenizer_empty() {
        let t = super::Xm85Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 85 ----

    #[test]
    fn xn_85_fenwick_prefix_sum() {
        let mut ft = super::Xn85Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_85_fenwick_range_sum() {
        let mut ft = super::Xn85Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_85_fenwick_point_query() {
        let mut ft = super::Xn85Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_85_fenwick_len() {
        let ft = super::Xn85Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_85_fenwick_multiple_updates() {
        let mut ft = super::Xn85Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_85_fenwick_single_element() {
        let mut ft = super::Xn85Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_85_fenwick_find_kth() {
        let mut ft = super::Xn85Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_85_fenwick_negative_delta() {
        let mut ft = super::Xn85Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 85 ----

    #[test]
    fn xn_85_avl_insert_get() {
        let mut m = super::Xn85AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_85_avl_remove() {
        let mut m = super::Xn85AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_85_avl_in_order() {
        let mut m = super::Xn85AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_85_avl_min_max() {
        let mut m = super::Xn85AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_85_avl_floor_ceiling() {
        let mut m = super::Xn85AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_85_avl_height_balanced() {
        let mut m = super::Xn85AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_85_avl_overwrite() {
        let mut m = super::Xn85AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_85_avl_empty() {
        let m: super::Xn85AVL<i32, i32> = super::Xn85AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo85RedBlack tests ---

    #[test]
    fn xo_85_rb_insert_and_get() {
        let mut tree = super::Xo85RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_85_rb_len_and_empty() {
        let mut tree = super::Xo85RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_85_rb_min_max() {
        let mut tree = super::Xo85RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_85_rb_contains() {
        let mut tree = super::Xo85RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_85_rb_remove() {
        let mut tree = super::Xo85RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_85_rb_in_order() {
        let mut tree = super::Xo85RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_85_rb_black_height() {
        let mut tree = super::Xo85RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_85_rb_overwrite() {
        let mut tree = super::Xo85RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo85ConsistentHash tests ---

    #[test]
    fn xo_85_ch_add_and_count() {
        let mut ring = super::Xo85ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_85_ch_remove_node() {
        let mut ring = super::Xo85ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_85_ch_get_node() {
        let mut ring = super::Xo85ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_85_ch_empty_ring() {
        let ring = super::Xo85ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_85_ch_distribution() {
        let mut ring = super::Xo85ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_85_ch_rebalance() {
        let mut ring = super::Xo85ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_85_ch_virtual_nodes() {
        let mut ring = super::Xo85ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_85_ch_consistent_lookup() {
        let mut ring = super::Xo85ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_85_splay_insert_get() {
        let mut t = super::Xp85SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_85_splay_remove() {
        let mut t = super::Xp85SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_85_splay_count_increases() {
        let mut t = super::Xp85SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_85_splay_depth() {
        let mut t = super::Xp85SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_85_splay_len_empty() {
        let t = super::Xp85SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_85_splay_min_max() {
        let mut t = super::Xp85SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_85_splay_overwrite() {
        let mut t = super::Xp85SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_85_splay_remove_missing() {
        let mut t = super::Xp85SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_85 treap tests ----
    #[test]
    fn xq_85_treap_empty() {
        let t = super::Xq85Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_85_treap_insert_get() {
        let mut t = super::Xq85Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_85_treap_overwrite() {
        let mut t = super::Xq85Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_85_treap_remove() {
        let mut t = super::Xq85Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_85_treap_min_max() {
        let mut t = super::Xq85Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_85_treap_rank() {
        let mut t = super::Xq85Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_85_treap_kth() {
        let mut t = super::Xq85Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_85_treap_in_order() {
        let mut t = super::Xq85Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_85 VEB tree tests ----
    #[test]
    fn xq_85_veb_empty() {
        let v = super::Xq85VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_85_veb_insert_contains() {
        let mut v = super::Xq85VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_85_veb_min_max() {
        let mut v = super::Xq85VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_85_veb_delete() {
        let mut v = super::Xq85VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_85_veb_successor() {
        let mut v = super::Xq85VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_85_veb_predecessor() {
        let mut v = super::Xq85VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_85_veb_count() {
        let mut v = super::Xq85VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_85_veb_duplicate_insert() {
        let mut v = super::Xq85VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_85_kdtree_empty() {
        let tree = super::Xr85KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_85_kdtree_insert_one() {
        let mut tree = super::Xr85KDTree::xr_new();
        tree.xr_insert(super::Xr85KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_85_kdtree_insert_multiple() {
        let mut tree = super::Xr85KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr85KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_85_kdtree_nearest_neighbor() {
        let mut tree = super::Xr85KDTree::xr_new();
        tree.xr_insert(super::Xr85KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr85KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr85KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_85_kdtree_nn_empty() {
        let tree = super::Xr85KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr85KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_85_kdtree_range_search() {
        let mut tree = super::Xr85KDTree::xr_new();
        tree.xr_insert(super::Xr85KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr85KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr85KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_85_kdtree_range_empty() {
        let mut tree = super::Xr85KDTree::xr_new();
        tree.xr_insert(super::Xr85KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_85_kdtree_all_points() {
        let mut tree = super::Xr85KDTree::xr_new();
        tree.xr_insert(super::Xr85KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr85KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_85_kdtree_depth() {
        let mut tree = super::Xr85KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr85KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_85_kdtree_bounding_box() {
        let mut tree = super::Xr85KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr85KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr85KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_85_persistent_array_new() {
        let arr = super::Xs85PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_85_persistent_array_push() {
        let mut arr = super::Xs85PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_85_persistent_array_set() {
        let mut arr = super::Xs85PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_85_persistent_array_diff() {
        let mut arr = super::Xs85PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_85_persistent_array_rollback() {
        let mut arr = super::Xs85PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_85_persistent_array_history() {
        let mut arr = super::Xs85PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_85_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs85PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_85_persistent_array_from_vec() {
        let arr = super::Xs85PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_85_concurrent_queue_new() {
        let q = super::Xs85ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_85_concurrent_queue_push_pop() {
        let mut q = super::Xs85ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_85_concurrent_queue_full() {
        let mut q = super::Xs85ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_85_concurrent_queue_drain() {
        let mut q = super::Xs85ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_85_concurrent_queue_try_pop() {
        let mut q = super::Xs85ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_85_concurrent_queue_clear() {
        let mut q = super::Xs85ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_85_range_map_new() {
        let rm = super::Xs85RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_85_range_map_insert_get() {
        let mut rm = super::Xs85RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_85_range_map_overlap() {
        let mut rm = super::Xs85RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_85_range_map_remove() {
        let mut rm = super::Xs85RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_85_range_map_gaps() {
        let mut rm = super::Xs85RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_85_range_map_coverage() {
        let mut rm = super::Xs85RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_85_range_map_contains() {
        let mut rm = super::Xs85RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_85_range_map_clear() {
        let mut rm = super::Xs85RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_85_circular_buffer_new() {
        let buf = super::Xs85CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_85_circular_buffer_push_pop() {
        let mut buf = super::Xs85CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_85_circular_buffer_overwrite() {
        let mut buf = super::Xs85CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_85_circular_buffer_peek() {
        let mut buf = super::Xs85CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_85_circular_buffer_is_full() {
        let mut buf = super::Xs85CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_85_circular_buffer_iter() {
        let mut buf = super::Xs85CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_85_circular_buffer_clear() {
        let mut buf = super::Xs85CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_85_circular_buffer_to_vec() {
        let mut buf = super::Xs85CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_85_stats_tracker_new() {
        let tracker = super::Xs85StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_85_stats_tracker_mean() {
        let mut tracker = super::Xs85StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_85_stats_tracker_min_max() {
        let mut tracker = super::Xs85StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_85_stats_tracker_median() {
        let mut tracker = super::Xs85StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_85_stats_tracker_variance() {
        let mut tracker = super::Xs85StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_85_stats_tracker_range() {
        let mut tracker = super::Xs85StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_85_stats_tracker_clear() {
        let mut tracker = super::Xs85StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_85_stats_tracker_sum() {
        let mut tracker = super::Xs85StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }

}
