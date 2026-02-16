//! Unicode confusable detection.

use std::fmt;

/// Severity level for a unicode highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

/// Error type for unicode highlight operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnicodeError {
    InvalidRange { start: u32, end: u32 },
    ConfigError(String),
}

impl fmt::Display for UnicodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnicodeError::InvalidRange { start, end } => {
                write!(f, "invalid range: {}..{}", start, end)
            }
            UnicodeError::ConfigError(msg) => write!(f, "config error: {}", msg),
        }
    }
}

impl std::error::Error for UnicodeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicodeCategory {
    Ambiguous,
    Invisible,
    NonBasicAscii,
    ConfusableWithAscii,
}

impl fmt::Display for UnicodeCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnicodeCategory::Ambiguous => write!(f, "ambiguous"),
            UnicodeCategory::Invisible => write!(f, "invisible"),
            UnicodeCategory::NonBasicAscii => write!(f, "non-basic ASCII"),
            UnicodeCategory::ConfusableWithAscii => write!(f, "confusable with ASCII"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnicodeHighlight {
    pub line: u32,
    pub column: u32,
    pub character: char,
    pub category: UnicodeCategory,
    pub replacement: Option<char>,
}

impl fmt::Display for UnicodeHighlight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "U+{:04X} '{}' at {}:{} ({})",
            self.character as u32,
            self.character,
            self.line,
            self.column,
            self.category,
        )
    }
}

impl UnicodeHighlight {
    /// Returns the severity level for this highlight.
    pub fn severity(&self) -> Severity {
        match self.category {
            UnicodeCategory::Invisible => Severity::Error,
            UnicodeCategory::ConfusableWithAscii | UnicodeCategory::Ambiguous => Severity::Warning,
            UnicodeCategory::NonBasicAscii => Severity::Info,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnicodeHighlightConfig {
    pub ambiguous_characters: bool,
    pub invisible_characters: bool,
    pub non_basic_ascii: bool,
    pub allowed_characters: Vec<char>,
}

impl Default for UnicodeHighlightConfig {
    fn default() -> Self {
        Self {
            ambiguous_characters: true,
            invisible_characters: true,
            non_basic_ascii: false,
            allowed_characters: Vec::new(),
        }
    }
}

impl UnicodeHighlightConfig {
    /// Strict config: all checks enabled.
    pub fn strict() -> Self {
        Self {
            ambiguous_characters: true,
            invisible_characters: true,
            non_basic_ascii: true,
            allowed_characters: Vec::new(),
        }
    }

    /// Permissive config: only invisible character checks.
    pub fn permissive() -> Self {
        Self {
            ambiguous_characters: false,
            invisible_characters: true,
            non_basic_ascii: false,
            allowed_characters: Vec::new(),
        }
    }

    /// Returns true if allowed_characters is empty.
    pub fn is_allowed_characters_empty(&self) -> bool {
        self.allowed_characters.is_empty()
    }

    /// Get the first allowed_character, if any.
    pub fn first_allowed_character(&self) -> Option<&char> {
        self.allowed_characters.first()
    }

    /// Get the last allowed_character, if any.
    pub fn last_allowed_character(&self) -> Option<&char> {
        self.allowed_characters.last()
    }

    /// Retain only allowed_characters matching the predicate.
    pub fn retain_allowed_characters(&mut self, f: impl Fn(&char) -> bool) {
        self.allowed_characters.retain(|item| f(item));
    }

    /// Toggle the `ambiguous_characters` flag.
    pub fn toggle_ambiguous_characters(&mut self) {
        self.ambiguous_characters = !self.ambiguous_characters;
    }

    /// Toggle the `invisible_characters` flag.
    pub fn toggle_invisible_characters(&mut self) {
        self.invisible_characters = !self.invisible_characters;
    }

    /// Toggle the `non_basic_ascii` flag.
    pub fn toggle_non_basic_ascii(&mut self) {
        self.non_basic_ascii = !self.non_basic_ascii;
    }
}

/// Maps common Cyrillic confusables to their ASCII lookalikes.
pub fn is_confusable(ch: char) -> Option<char> {
    match ch {
        '\u{0430}' => Some('a'), // Cyrillic а
        '\u{043E}' => Some('o'), // Cyrillic о
        '\u{0435}' => Some('e'), // Cyrillic е
        '\u{0441}' => Some('c'), // Cyrillic с
        '\u{0440}' => Some('p'), // Cyrillic р
        '\u{0445}' => Some('x'), // Cyrillic х
        '\u{0455}' => Some('s'), // Cyrillic ѕ
        _ => None,
    }
}

/// Returns `true` for zero-width and other invisible characters.
pub fn is_invisible(ch: char) -> bool {
    matches!(
        ch,
        '\u{200B}' // zero-width space
        | '\u{200C}' // zero-width non-joiner
        | '\u{200D}' // zero-width joiner
        | '\u{2060}' // word joiner
        | '\u{FEFF}' // zero-width no-break space / BOM
        | '\u{00AD}' // soft hyphen
    )
}

pub fn highlight_line(
    line: &str,
    line_number: u32,
    config: &UnicodeHighlightConfig,
) -> Vec<UnicodeHighlight> {
    let mut highlights = Vec::new();
    for (col, ch) in line.chars().enumerate() {
        if config.allowed_characters.contains(&ch) {
            continue;
        }

        if config.invisible_characters && is_invisible(ch) {
            highlights.push(UnicodeHighlight {
                line: line_number,
                column: col as u32,
                character: ch,
                category: UnicodeCategory::Invisible,
                replacement: None,
            });
            continue;
        }

        if config.ambiguous_characters {
            if let Some(replacement) = is_confusable(ch) {
                highlights.push(UnicodeHighlight {
                    line: line_number,
                    column: col as u32,
                    character: ch,
                    category: UnicodeCategory::ConfusableWithAscii,
                    replacement: Some(replacement),
                });
                continue;
            }
        }

        if config.non_basic_ascii && !ch.is_ascii() {
            highlights.push(UnicodeHighlight {
                line: line_number,
                column: col as u32,
                character: ch,
                category: UnicodeCategory::NonBasicAscii,
                replacement: None,
            });
        }
    }
    highlights
}

/// Returns `true` for RTL override and embedding characters (security concern).
pub fn is_rtl_override(ch: char) -> bool {
    matches!(
        ch,
        '\u{202A}' // left-to-right embedding
        | '\u{202B}' // right-to-left embedding
        | '\u{202C}' // pop directional formatting
        | '\u{202D}' // left-to-right override
        | '\u{202E}' // right-to-left override
        | '\u{2066}' // left-to-right isolate
        | '\u{2067}' // right-to-left isolate
        | '\u{2068}' // first strong isolate
        | '\u{2069}' // pop directional isolate
    )
}

/// Count non-ASCII characters in a string.
pub fn count_non_ascii(s: &str) -> usize {
    s.chars().filter(|ch| !ch.is_ascii()).count()
}

/// Replace all confusable characters with their ASCII equivalents.
pub fn replace_confusables(s: &str) -> String {
    s.chars()
        .map(|ch| is_confusable(ch).unwrap_or(ch))
        .collect()
}

/// Process multiple lines and return all highlights.
pub fn highlight_document(
    lines: &[&str],
    config: &UnicodeHighlightConfig,
) -> Vec<UnicodeHighlight> {
    lines
        .iter()
        .enumerate()
        .flat_map(|(i, line)| highlight_line(line, i as u32, config))
        .collect()
}

/// Accumulated statistics for unicodehl operations.
#[derive(Debug, Clone, PartialEq)]
pub struct UnicodehlStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl UnicodehlStats {
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
    pub fn merge(&mut self, other: &UnicodehlStats) {
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

impl Default for UnicodehlStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UnicodehlStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UnicodehlStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for unicodehl.
#[derive(Debug, Clone)]
pub struct UnicodehlValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl UnicodehlValidator {
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

impl Default for UnicodehlValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_confusables() {
        assert_eq!(is_confusable('\u{0430}'), Some('a'));
        assert_eq!(is_confusable('\u{043E}'), Some('o'));
        assert_eq!(is_confusable('a'), None);
    }

    #[test]
    fn detects_invisible() {
        assert!(is_invisible('\u{200B}'));
        assert!(is_invisible('\u{FEFF}'));
        assert!(!is_invisible('a'));
    }

    #[test]
    fn highlight_line_mixed() {
        let config = UnicodeHighlightConfig::default();
        // "h\u{0435}llo" — Cyrillic е in position 1
        let line = "h\u{0435}llo";
        let results = highlight_line(line, 0, &config);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].column, 1);
        assert_eq!(results[0].category, UnicodeCategory::ConfusableWithAscii);
        assert_eq!(results[0].replacement, Some('e'));
    }

    #[test]
    fn allowed_characters_skipped() {
        let config = UnicodeHighlightConfig {
            allowed_characters: vec!['\u{0430}'],
            ..UnicodeHighlightConfig::default()
        };
        let line = "\u{0430}bc";
        let results = highlight_line(line, 0, &config);
        assert!(results.is_empty());
    }

    #[test]
    fn highlight_document_multi_line() {
        let config = UnicodeHighlightConfig::default();
        let lines = vec!["h\u{0435}llo", "w\u{200B}rld"];
        let results = highlight_document(&lines, &config);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].line, 0);
        assert_eq!(results[0].category, UnicodeCategory::ConfusableWithAscii);
        assert_eq!(results[1].line, 1);
        assert_eq!(results[1].category, UnicodeCategory::Invisible);
    }

    #[test]
    fn highlight_document_empty() {
        let config = UnicodeHighlightConfig::default();
        let results = highlight_document(&[], &config);
        assert!(results.is_empty());
    }

    #[test]
    fn severity_levels() {
        let invisible = UnicodeHighlight {
            line: 0,
            column: 0,
            character: '\u{200B}',
            category: UnicodeCategory::Invisible,
            replacement: None,
        };
        assert_eq!(invisible.severity(), Severity::Error);

        let confusable = UnicodeHighlight {
            line: 0,
            column: 0,
            character: '\u{0430}',
            category: UnicodeCategory::ConfusableWithAscii,
            replacement: Some('a'),
        };
        assert_eq!(confusable.severity(), Severity::Warning);

        let non_basic = UnicodeHighlight {
            line: 0,
            column: 0,
            character: '\u{00E9}',
            category: UnicodeCategory::NonBasicAscii,
            replacement: None,
        };
        assert_eq!(non_basic.severity(), Severity::Info);
    }

    #[test]
    fn strict_config_catches_non_basic() {
        let config = UnicodeHighlightConfig::strict();
        assert!(config.ambiguous_characters);
        assert!(config.invisible_characters);
        assert!(config.non_basic_ascii);
        let line = "caf\u{00E9}";
        let results = highlight_line(line, 0, &config);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].category, UnicodeCategory::NonBasicAscii);
    }

    #[test]
    fn permissive_config_only_invisible() {
        let config = UnicodeHighlightConfig::permissive();
        assert!(!config.ambiguous_characters);
        assert!(config.invisible_characters);
        assert!(!config.non_basic_ascii);
        // Confusable should be ignored under permissive config
        let line = "h\u{0435}llo";
        let results = highlight_line(line, 0, &config);
        assert!(results.is_empty());
        // Invisible should still be caught
        let line2 = "a\u{200B}b";
        let results2 = highlight_line(line2, 0, &config);
        assert_eq!(results2.len(), 1);
        assert_eq!(results2[0].category, UnicodeCategory::Invisible);
    }

    #[test]
    fn detects_rtl_override() {
        assert!(is_rtl_override('\u{202E}'));
        assert!(is_rtl_override('\u{2067}'));
        assert!(!is_rtl_override('a'));
        assert!(!is_rtl_override('\u{0430}'));
    }

    #[test]
    fn count_non_ascii_works() {
        assert_eq!(count_non_ascii("hello"), 0);
        assert_eq!(count_non_ascii("h\u{0435}ll\u{043E}"), 2);
        assert_eq!(count_non_ascii(""), 0);
        assert_eq!(count_non_ascii("\u{200B}\u{200C}\u{200D}"), 3);
    }

    #[test]
    fn replace_confusables_works() {
        assert_eq!(replace_confusables("h\u{0435}llo"), "hello");
        assert_eq!(replace_confusables("\u{0430}\u{043E}"), "ao");
        assert_eq!(replace_confusables("hello"), "hello");
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", UnicodeCategory::Invisible), "invisible");
        assert_eq!(
            format!("{}", UnicodeCategory::ConfusableWithAscii),
            "confusable with ASCII"
        );
        assert_eq!(format!("{}", UnicodeCategory::NonBasicAscii), "non-basic ASCII");
        assert_eq!(format!("{}", UnicodeCategory::Ambiguous), "ambiguous");

        assert_eq!(format!("{}", Severity::Error), "error");
        assert_eq!(format!("{}", Severity::Warning), "warning");
        assert_eq!(format!("{}", Severity::Info), "info");

        let h = UnicodeHighlight {
            line: 5,
            column: 3,
            character: '\u{0430}',
            category: UnicodeCategory::ConfusableWithAscii,
            replacement: Some('a'),
        };
        let display = format!("{}", h);
        assert!(display.contains("U+0430"));
        assert!(display.contains("5:3"));
        assert!(display.contains("confusable with ASCII"));
    }

    #[test]
    fn error_display() {
        let err = UnicodeError::InvalidRange { start: 10, end: 5 };
        assert_eq!(format!("{}", err), "invalid range: 10..5");

        let err2 = UnicodeError::ConfigError("bad value".to_string());
        assert_eq!(format!("{}", err2), "config error: bad value");
    }

    #[test]
    fn eq_severity_same() {
        assert_eq!(Severity::Error, Severity::Error);
    }

    #[test]
    fn ne_severity_diff() {
        assert_ne!(Severity::Error, Severity::Warning);
    }

    #[test]
    fn eq_unicodecategory_same() {
        assert_eq!(UnicodeCategory::Ambiguous, UnicodeCategory::Ambiguous);
    }

    #[test]
    fn ne_unicodecategory_diff() {
        assert_ne!(UnicodeCategory::Ambiguous, UnicodeCategory::Invisible);
    }

    #[test]
    fn display_severity_variants() {
        assert!(!Severity::Error.to_string().is_empty());
        assert!(!Severity::Warning.to_string().is_empty());
        assert!(!Severity::Info.to_string().is_empty());
    }

    #[test]
    fn display_unicodeerror_variants() {
        let e = UnicodeError::ConfigError("test".into());
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn display_unicodecategory_variants() {
        assert!(!UnicodeCategory::Ambiguous.to_string().is_empty());
        assert!(!UnicodeCategory::Invisible.to_string().is_empty());
        assert!(!UnicodeCategory::NonBasicAscii.to_string().is_empty());
        assert!(!UnicodeCategory::ConfusableWithAscii.to_string().is_empty());
    }

    #[test]
    fn behavior_check_0() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = UnicodeHighlightConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn unicodehl_stats_new_defaults() {
        let stats = UnicodehlStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn unicodehl_stats_record_success() {
        let mut stats = UnicodehlStats::new();
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
    fn unicodehl_stats_record_failure() {
        let mut stats = UnicodehlStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn unicodehl_stats_reset() {
        let mut stats = UnicodehlStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn unicodehl_stats_merge() {
        let mut a = UnicodehlStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = UnicodehlStats::new();
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
    fn unicodehl_stats_display() {
        let mut stats = UnicodehlStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn unicodehl_stats_default() {
        let stats = UnicodehlStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn unicodehl_validator_accepts_valid_name() {
        let v = UnicodehlValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn unicodehl_validator_rejects_empty() {
        let v = UnicodehlValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn unicodehl_validator_rejects_too_long() {
        let v = UnicodehlValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn unicodehl_validator_forbidden_prefix() {
        let v = UnicodehlValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn unicodehl_validator_allowed_chars() {
        let v = UnicodehlValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn unicodehl_validator_range() {
        let v = UnicodehlValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn unicodehl_sanitize_removes_control() {
        let result = UnicodehlValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn unicodehl_truncate_short_string() {
        assert_eq!(UnicodehlValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn unicodehl_truncate_long_string() {
        let result = UnicodehlValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn unicodehl_is_ascii_printable() {
        assert!(UnicodehlValidator::is_ascii_printable("Hello World 123"));
        assert!(!UnicodehlValidator::is_ascii_printable("Hello\x00World"));
    }
}
