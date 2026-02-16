//! Glob pattern matching.
//!
//! Wraps the `globset` crate to provide VS Code-compatible glob matching.

use globset::{Glob, GlobMatcher, GlobSet, GlobSetBuilder};

/// A compiled glob pattern for matching file paths.
pub struct GlobPattern {
    matcher: GlobMatcher,
    pattern: String,
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
}

/// A set of glob patterns compiled for efficient matching.
pub struct GlobPatternSet {
    set: GlobSet,
    patterns: Vec<String>,
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
}
