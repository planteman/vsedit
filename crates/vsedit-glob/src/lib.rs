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
}
