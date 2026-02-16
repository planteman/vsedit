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
}
