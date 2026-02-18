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

}
