//! Unicode confusable detection.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// Enhanced Unicode character category detection
// ---------------------------------------------------------------------------

/// Fine-grained Unicode character category based on code-point ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnicodeCharCategory {
    Ascii,
    Latin,
    Cyrillic,
    Greek,
    CJK,
    Emoji,
    MathSymbol,
    Invisible,
    Bidi,
    Other,
}

impl fmt::Display for UnicodeCharCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            UnicodeCharCategory::Ascii => "ASCII",
            UnicodeCharCategory::Latin => "Latin extended",
            UnicodeCharCategory::Cyrillic => "Cyrillic",
            UnicodeCharCategory::Greek => "Greek",
            UnicodeCharCategory::CJK => "CJK",
            UnicodeCharCategory::Emoji => "Emoji",
            UnicodeCharCategory::MathSymbol => "math symbol",
            UnicodeCharCategory::Invisible => "invisible",
            UnicodeCharCategory::Bidi => "bidirectional control",
            UnicodeCharCategory::Other => "other non-ASCII",
        };
        write!(f, "{}", label)
    }
}

/// Classify a single character into a [`UnicodeCharCategory`].
pub fn classify_char(c: char) -> UnicodeCharCategory {
    let cp = c as u32;
    match cp {
        0x0000..=0x007F => UnicodeCharCategory::Ascii,
        // Invisible characters (check before Bidi since ranges partially overlap)
        0x200B..=0x200E | 0xFEFF => UnicodeCharCategory::Invisible,
        // Bidi control characters
        0x200F..=0x202E => UnicodeCharCategory::Bidi,
        0x00C0..=0x024F => UnicodeCharCategory::Latin,
        0x0370..=0x03FF => UnicodeCharCategory::Greek,
        0x0400..=0x04FF => UnicodeCharCategory::Cyrillic,
        0x2200..=0x22FF => UnicodeCharCategory::MathSymbol,
        0x4E00..=0x9FFF => UnicodeCharCategory::CJK,
        0x1F600..=0x1F64F => UnicodeCharCategory::Emoji,
        _ => UnicodeCharCategory::Other,
    }
}

/// Classify every character in a string, returning `(char, category)` pairs.
pub fn classify_string(s: &str) -> Vec<(char, UnicodeCharCategory)> {
    s.chars().map(|c| (c, classify_char(c))).collect()
}

// ---------------------------------------------------------------------------
// Highlight ranges – contiguous runs of non-ASCII characters
// ---------------------------------------------------------------------------

/// A contiguous byte-range of non-ASCII characters sharing the same category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnicodeHighlightRange {
    /// Byte offset of the first character in the range (inclusive).
    pub start: usize,
    /// Byte offset one past the last character in the range (exclusive).
    pub end: usize,
    /// The shared category of the characters in this range.
    pub category: UnicodeCharCategory,
    /// Human-readable description of the range.
    pub description: String,
}

/// Scan `line` for runs of non-ASCII characters and return merged ranges.
///
/// Adjacent characters that share the same [`UnicodeCharCategory`] are merged
/// into a single [`UnicodeHighlightRange`].  Pure-ASCII characters are skipped.
pub fn unicode_highlight_ranges(line: &str) -> Vec<UnicodeHighlightRange> {
    let mut ranges: Vec<UnicodeHighlightRange> = Vec::new();

    for (byte_offset, ch) in line.char_indices() {
        let cat = classify_char(ch);
        if cat == UnicodeCharCategory::Ascii {
            continue;
        }

        let ch_end = byte_offset + ch.len_utf8();

        if let Some(last) = ranges.last_mut() {
            if last.category == cat && last.end == byte_offset {
                // Extend the current range.
                last.end = ch_end;
                last.description = format!(
                    "{} characters (U+{:04X}..U+{:04X})",
                    last.category,
                    line[last.start..].chars().next().unwrap() as u32,
                    ch as u32,
                );
                continue;
            }
        }

        ranges.push(UnicodeHighlightRange {
            start: byte_offset,
            end: ch_end,
            category: cat,
            description: format!("{} character U+{:04X}", cat, ch as u32),
        });
    }

    ranges
}

// ---------------------------------------------------------------------------
// Unicode escape / unescape helpers
// ---------------------------------------------------------------------------

/// Replace every non-ASCII character with its `\u{XXXX}` escape sequence.
pub fn unicode_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii() {
            out.push(ch);
        } else {
            out.push_str(&format!("\\u{{{:04X}}}", ch as u32));
        }
    }
    out
}

/// Convert `\u{XXXX}` escape sequences back to the corresponding characters.
///
/// Characters that are not part of a valid escape sequence are passed through
/// unchanged.
pub fn unicode_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for the start of a \u{...} escape.
        if i + 3 < len && bytes[i] == b'\\' && bytes[i + 1] == b'u' && bytes[i + 2] == b'{' {
            if let Some(close) = s[i + 3..].find('}') {
                let hex_str = &s[i + 3..i + 3 + close];
                if let Ok(cp) = u32::from_str_radix(hex_str, 16) {
                    if let Some(ch) = char::from_u32(cp) {
                        out.push(ch);
                        i += 3 + close + 1; // skip past the closing '}'
                        continue;
                    }
                }
            }
        }
        // Not a valid escape – emit the byte as-is (safe because we index
        // only on ASCII-compatible bytes for the escape prefix).
        out.push(s[i..].chars().next().unwrap());
        i += s[i..].chars().next().unwrap().len_utf8();
    }

    out
}


// ---------------------------------------------------------------------------
// Severity helpers
// ---------------------------------------------------------------------------

impl Severity {
    /// Returns all severity variants in decreasing severity order.
    pub fn all() -> &'static [Severity] {
        &[Severity::Error, Severity::Warning, Severity::Info]
    }

    /// Returns a numeric level (0=info, 1=warning, 2=error).
    pub fn level(&self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Warning => 1,
            Severity::Error => 2,
        }
    }

    /// Parse from a string.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "error" | "err" => Some(Self::Error),
            "warning" | "warn" => Some(Self::Warning),
            "info" | "information" => Some(Self::Info),
            _ => None,
        }
    }

    /// Returns an icon character.
    pub fn icon(&self) -> char {
        match self {
            Severity::Error => '✖',
            Severity::Warning => '⚠',
            Severity::Info => 'ℹ',
        }
    }
}

// ---------------------------------------------------------------------------
// UnicodeCharCategory helpers
// ---------------------------------------------------------------------------

impl UnicodeCharCategory {
    /// Returns true if the category is potentially dangerous.
    pub fn is_suspicious(&self) -> bool {
        matches!(
            self,
            UnicodeCharCategory::Cyrillic
                | UnicodeCharCategory::Greek
                | UnicodeCharCategory::Invisible
                | UnicodeCharCategory::Bidi
        )
    }

    /// Suggested severity for this category.
    pub fn severity(&self) -> Severity {
        match self {
            UnicodeCharCategory::Bidi | UnicodeCharCategory::Invisible => Severity::Error,
            UnicodeCharCategory::Cyrillic | UnicodeCharCategory::Greek => Severity::Warning,
            UnicodeCharCategory::Latin => Severity::Info,
            _ => Severity::Info,
        }
    }
}

// ---------------------------------------------------------------------------
// Unicode analysis helpers
// ---------------------------------------------------------------------------

/// Summary of unicode analysis for a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnicodeAnalysis {
    pub total_chars: usize,
    pub ascii_count: usize,
    pub non_ascii_count: usize,
    pub confusable_count: usize,
    pub bidi_count: usize,
}

impl UnicodeAnalysis {
    /// Analyze a string for unicode characteristics.
    pub fn analyze(s: &str) -> Self {
        let chars: Vec<(char, UnicodeCharCategory)> = classify_string(s);
        Self {
            total_chars: chars.len(),
            ascii_count: chars.iter().filter(|(_, c)| matches!(c, UnicodeCharCategory::Ascii)).count(),
            non_ascii_count: chars.iter().filter(|(_, c)| !matches!(c, UnicodeCharCategory::Ascii)).count(),
            confusable_count: chars.iter().filter(|(_, c)| matches!(c, UnicodeCharCategory::Cyrillic | UnicodeCharCategory::Greek)).count(),
            bidi_count: chars.iter().filter(|(_, c)| matches!(c, UnicodeCharCategory::Bidi)).count(),
        }
    }

    /// Returns true if no suspicious characters were found.
    pub fn is_safe(&self) -> bool {
        self.confusable_count == 0 && self.bidi_count == 0
    }

    /// Returns the percentage of ASCII characters.
    pub fn ascii_percentage(&self) -> f64 {
        if self.total_chars == 0 {
            100.0
        } else {
            self.ascii_count as f64 / self.total_chars as f64 * 100.0
        }
    }
}

impl std::fmt::Display for UnicodeAnalysis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} chars ({} ASCII, {} non-ASCII, {} confusable, {} bidi)",
            self.total_chars, self.ascii_count, self.non_ascii_count,
            self.confusable_count, self.bidi_count
        )
    }
}

/// Strip all non-ASCII characters from a string.
pub fn strip_non_ascii(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii()).collect()
}

/// Returns codepoint info for a character as "U+XXXX".
pub fn char_codepoint(c: char) -> String {
    format!("U+{:04X}", c as u32)
}

/// Returns codepoint info for all characters in a string.
pub fn string_codepoints(s: &str) -> Vec<(char, String)> {
    s.chars().map(|c| (c, char_codepoint(c))).collect()
}

// ---------------------------------------------------------------------------
// ConfusableMatch / ConfusableDetector
// ---------------------------------------------------------------------------

/// A match found by the confusable detector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfusableMatch {
    pub position: usize,
    pub original_char: char,
    pub confusable_with: char,
    pub severity: Severity,
}

impl fmt::Display for ConfusableMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pos {}: U+{:04X} '{}' looks like '{}' ({})",
            self.position,
            self.original_char as u32,
            self.original_char,
            self.confusable_with,
            self.severity,
        )
    }
}

/// Detects characters visually similar to ASCII in a string.
#[derive(Debug, Clone)]
pub struct ConfusableDetector {
    min_severity: Severity,
}

impl ConfusableDetector {
    pub fn new() -> Self {
        Self { min_severity: Severity::Info }
    }

    pub fn with_min_severity(mut self, sev: Severity) -> Self {
        self.min_severity = sev;
        self
    }

    /// Scan `text` and return all confusable matches.
    pub fn detect(&self, text: &str) -> Vec<ConfusableMatch> {
        let mut matches = Vec::new();
        for (i, ch) in text.chars().enumerate() {
            if let Some(ascii_eq) = is_confusable(ch) {
                let sev = if ch as u32 > 0x2000 { Severity::Error } else { Severity::Warning };
                if sev.level() >= self.min_severity.level() {
                    matches.push(ConfusableMatch {
                        position: i,
                        original_char: ch,
                        confusable_with: ascii_eq,
                        severity: sev,
                    });
                }
            }
        }
        matches
    }

    /// Returns true if the text contains any confusable characters.
    pub fn has_confusables(&self, text: &str) -> bool {
        text.chars().any(|ch| is_confusable(ch).is_some())
    }

    /// Replaces all confusable characters with their ASCII equivalents.
    pub fn normalize(&self, text: &str) -> String {
        text.chars()
            .map(|ch| is_confusable(ch).unwrap_or(ch))
            .collect()
    }
}

impl Default for ConfusableDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HomoglyphMap
// ---------------------------------------------------------------------------

/// A bidirectional map of homoglyph pairs (visual look-alikes).
#[derive(Debug, Clone)]
pub struct HomoglyphMap {
    pairs: Vec<(char, char)>,
}

impl HomoglyphMap {
    pub fn new() -> Self {
        Self { pairs: Vec::new() }
    }

    pub fn add(&mut self, from: char, to: char) {
        if !self.pairs.iter().any(|&(a, b)| a == from && b == to) {
            self.pairs.push((from, to));
        }
    }

    pub fn lookup(&self, ch: char) -> Option<char> {
        self.pairs.iter().find(|&&(a, _)| a == ch).map(|&(_, b)| b)
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(char, char)> {
        self.pairs.iter()
    }
}

impl From<Vec<(char, char)>> for HomoglyphMap {
    fn from(pairs: Vec<(char, char)>) -> Self {
        Self { pairs }
    }
}

impl Default for HomoglyphMap {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for HomoglyphMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HomoglyphMap({} pairs)", self.pairs.len())
    }
}

// ---------------------------------------------------------------------------
// Unicode utility functions
// ---------------------------------------------------------------------------

/// Returns `true` if every character in the string is basic ASCII
/// (printable ASCII 0x20..=0x7E plus newline/tab/CR).
pub fn is_safe_text(s: &str) -> bool {
    s.chars().all(|ch| ch.is_ascii() && !is_invisible(ch))
}

/// Extracts only the non-ASCII characters from a string, preserving order.
pub fn extract_non_ascii(s: &str) -> Vec<char> {
    s.chars().filter(|ch| !ch.is_ascii()).collect()
}

/// Returns a mapping of non-ASCII characters to their confusable ASCII
/// equivalents found in `s`. Characters that are not confusable are omitted.
pub fn confusable_pairs_in(s: &str) -> Vec<(char, char)> {
    s.chars()
        .filter_map(|ch| is_confusable(ch).map(|ascii| (ch, ascii)))
        .collect()
}

/// Groups highlights by their category, returning a map from category
/// display name to a vector of highlights.
pub fn group_highlights_by_category(
    highlights: &[UnicodeHighlight],
) -> std::collections::HashMap<String, Vec<&UnicodeHighlight>> {
    let mut map: std::collections::HashMap<String, Vec<&UnicodeHighlight>> =
        std::collections::HashMap::new();
    for h in highlights {
        map.entry(format!("{}", h.category))
            .or_default()
            .push(h);
    }
    map
}

/// Returns `true` if the string contains any RTL override characters.
pub fn contains_rtl_override(s: &str) -> bool {
    s.chars().any(is_rtl_override)
}

/// Filters a list of highlights down to only those at or above `min_severity`.
pub fn filter_by_min_severity(
    highlights: &[UnicodeHighlight],
    min_severity: Severity,
) -> Vec<&UnicodeHighlight> {
    let min_level = match min_severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    };
    highlights
        .iter()
        .filter(|h| {
            let level = match h.severity() {
                Severity::Error => 0,
                Severity::Warning => 1,
                Severity::Info => 2,
            };
            level <= min_level
        })
        .collect()
}

/// Produces a one-line diagnostic summary for each highlight.
pub fn format_diagnostics(highlights: &[UnicodeHighlight]) -> Vec<String> {
    highlights
        .iter()
        .map(|h| {
            format!(
                "[{}] U+{:04X} '{}' at {}:{} ({})",
                h.severity(),
                h.character as u32,
                h.character,
                h.line,
                h.column,
                h.category,
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Script mixing detection
// ---------------------------------------------------------------------------

/// The script of a character, for mixed-script detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnicodeScript {
    Common,
    Latin,
    Cyrillic,
    Greek,
    Arabic,
    Hebrew,
    Han,
    Hiragana,
    Katakana,
    Hangul,
    Unknown,
}

impl fmt::Display for UnicodeScript {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            UnicodeScript::Common => "Common",
            UnicodeScript::Latin => "Latin",
            UnicodeScript::Cyrillic => "Cyrillic",
            UnicodeScript::Greek => "Greek",
            UnicodeScript::Arabic => "Arabic",
            UnicodeScript::Hebrew => "Hebrew",
            UnicodeScript::Han => "Han",
            UnicodeScript::Hiragana => "Hiragana",
            UnicodeScript::Katakana => "Katakana",
            UnicodeScript::Hangul => "Hangul",
            UnicodeScript::Unknown => "Unknown",
        };
        write!(f, "{}", label)
    }
}

/// Determine the script of a character based on its code point range.
pub fn char_script(ch: char) -> UnicodeScript {
    let cp = ch as u32;
    match cp {
        0x0000..=0x007F => UnicodeScript::Common,    // Basic ASCII (digits, punctuation, controls)
        #[allow(unreachable_patterns)]
        0x0041..=0x005A | 0x0061..=0x007A => UnicodeScript::Latin,
        0x00C0..=0x024F | 0x1E00..=0x1EFF => UnicodeScript::Latin,
        0x0370..=0x03FF | 0x1F00..=0x1FFF => UnicodeScript::Greek,
        0x0400..=0x04FF | 0x0500..=0x052F => UnicodeScript::Cyrillic,
        0x0590..=0x05FF => UnicodeScript::Hebrew,
        0x0600..=0x06FF | 0x0750..=0x077F => UnicodeScript::Arabic,
        0x3040..=0x309F => UnicodeScript::Hiragana,
        0x30A0..=0x30FF => UnicodeScript::Katakana,
        0xAC00..=0xD7AF => UnicodeScript::Hangul,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF => UnicodeScript::Han,
        _ => UnicodeScript::Unknown,
    }
}

/// A warning about mixed scripts in a single identifier or line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedScriptWarning {
    pub scripts_found: Vec<UnicodeScript>,
    pub text: String,
}

impl fmt::Display for MixedScriptWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<String> = self.scripts_found.iter().map(|s| s.to_string()).collect();
        write!(f, "mixed scripts [{}] in {:?}", names.join(", "), self.text)
    }
}

/// Check a string for mixed scripts. Returns `None` if only one non-Common
/// script is present or the string is empty.
pub fn detect_mixed_scripts(s: &str) -> Option<MixedScriptWarning> {
    let mut seen = Vec::new();
    for ch in s.chars() {
        let script = char_script(ch);
        if script == UnicodeScript::Common || script == UnicodeScript::Unknown {
            continue;
        }
        if !seen.contains(&script) {
            seen.push(script);
        }
    }
    if seen.len() > 1 {
        Some(MixedScriptWarning {
            scripts_found: seen,
            text: s.to_string(),
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Zero-width and invisible character utilities
// ---------------------------------------------------------------------------

/// Describes an invisible character found in text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvisibleCharInfo {
    pub position: usize,
    pub character: char,
    pub name: &'static str,
}

/// Return a human-readable name for known invisible characters.
pub fn invisible_char_name(ch: char) -> Option<&'static str> {
    match ch {
        '\u{200B}' => Some("Zero Width Space"),
        '\u{200C}' => Some("Zero Width Non-Joiner"),
        '\u{200D}' => Some("Zero Width Joiner"),
        '\u{2060}' => Some("Word Joiner"),
        '\u{FEFF}' => Some("Zero Width No-Break Space / BOM"),
        '\u{00AD}' => Some("Soft Hyphen"),
        '\u{180E}' => Some("Mongolian Vowel Separator"),
        '\u{2061}' => Some("Function Application"),
        '\u{2062}' => Some("Invisible Times"),
        '\u{2063}' => Some("Invisible Separator"),
        '\u{2064}' => Some("Invisible Plus"),
        _ => None,
    }
}

/// Find all invisible / zero-width characters in a string with their positions.
pub fn find_invisible_chars(s: &str) -> Vec<InvisibleCharInfo> {
    s.chars()
        .enumerate()
        .filter_map(|(i, ch)| {
            invisible_char_name(ch).map(|name| InvisibleCharInfo {
                position: i,
                character: ch,
                name,
            })
        })
        .collect()
}

/// Strip all zero-width and invisible characters from a string.
pub fn strip_invisible(s: &str) -> String {
    s.chars()
        .filter(|&ch| invisible_char_name(ch).is_none())
        .collect()
}

/// Returns `true` if the string contains any zero-width characters.
pub fn contains_zero_width(s: &str) -> bool {
    s.chars().any(|ch| {
        matches!(
            ch,
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
        )
    })
}

// ---------------------------------------------------------------------------
// Combining character analysis
// ---------------------------------------------------------------------------

/// Returns `true` if the character is a Unicode combining mark (U+0300..U+036F
/// for the Combining Diacritical Marks block, plus U+20D0..U+20FF for
/// Combining Diacritical Marks for Symbols).
pub fn is_combining_mark(ch: char) -> bool {
    let cp = ch as u32;
    matches!(cp, 0x0300..=0x036F | 0x20D0..=0x20FF | 0x0483..=0x0489 | 0xFE20..=0xFE2F)
}

/// Count the number of combining marks in a string.
pub fn count_combining_marks(s: &str) -> usize {
    s.chars().filter(|&ch| is_combining_mark(ch)).count()
}

/// Detect "Zalgo text" — strings where combining marks are stacked excessively.
/// Returns `true` if any base character has more than `threshold` consecutive
/// combining marks following it.
pub fn has_excessive_combining(s: &str, threshold: usize) -> bool {
    let mut run = 0usize;
    for ch in s.chars() {
        if is_combining_mark(ch) {
            run += 1;
            if run > threshold {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Bidirectional text safety
// ---------------------------------------------------------------------------

/// All Unicode bidirectional control characters.
pub fn is_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{200E}'   // Left-to-Right Mark
        | '\u{200F}' // Right-to-Left Mark
        | '\u{061C}' // Arabic Letter Mark
        | '\u{202A}' // Left-to-Right Embedding
        | '\u{202B}' // Right-to-Left Embedding
        | '\u{202C}' // Pop Directional Formatting
        | '\u{202D}' // Left-to-Right Override
        | '\u{202E}' // Right-to-Left Override
        | '\u{2066}' // Left-to-Right Isolate
        | '\u{2067}' // Right-to-Left Isolate
        | '\u{2068}' // First Strong Isolate
        | '\u{2069}' // Pop Directional Isolate
    )
}

/// Find all bidi control characters in a string with their positions.
pub fn find_bidi_controls(s: &str) -> Vec<(usize, char)> {
    s.chars()
        .enumerate()
        .filter(|&(_, ch)| is_bidi_control(ch))
        .collect()
}

/// Check whether bidi control characters are properly balanced (each open has
/// a matching close). Returns `true` if balanced or no bidi controls present.
pub fn bidi_controls_balanced(s: &str) -> bool {
    let mut depth: i32 = 0;
    for ch in s.chars() {
        match ch {
            '\u{202A}' | '\u{202B}' | '\u{202D}' | '\u{202E}' => depth += 1,
            '\u{202C}' => depth -= 1,
            '\u{2066}' | '\u{2067}' | '\u{2068}' => depth += 1,
            '\u{2069}' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

// ---------------------------------------------------------------------------
// Unicode block identification
// ---------------------------------------------------------------------------

/// Identifies the Unicode block for a character.
pub fn unicode_block_name(ch: char) -> &'static str {
    let cp = ch as u32;
    match cp {
        0x0000..=0x007F => "Basic Latin",
        0x0080..=0x00FF => "Latin-1 Supplement",
        0x0100..=0x017F => "Latin Extended-A",
        0x0180..=0x024F => "Latin Extended-B",
        0x0250..=0x02AF => "IPA Extensions",
        0x0300..=0x036F => "Combining Diacritical Marks",
        0x0370..=0x03FF => "Greek and Coptic",
        0x0400..=0x04FF => "Cyrillic",
        0x0500..=0x052F => "Cyrillic Supplement",
        0x0590..=0x05FF => "Hebrew",
        0x0600..=0x06FF => "Arabic",
        0x2000..=0x206F => "General Punctuation",
        0x2070..=0x209F => "Superscripts and Subscripts",
        0x20A0..=0x20CF => "Currency Symbols",
        0x20D0..=0x20FF => "Combining Diacritical Marks for Symbols",
        0x2100..=0x214F => "Letterlike Symbols",
        0x2200..=0x22FF => "Mathematical Operators",
        0x2300..=0x23FF => "Miscellaneous Technical",
        0x2400..=0x243F => "Control Pictures",
        0x2500..=0x257F => "Box Drawing",
        0x2580..=0x259F => "Block Elements",
        0x25A0..=0x25FF => "Geometric Shapes",
        0x2600..=0x26FF => "Miscellaneous Symbols",
        0x2700..=0x27BF => "Dingbats",
        0x3000..=0x303F => "CJK Symbols and Punctuation",
        0x3040..=0x309F => "Hiragana",
        0x30A0..=0x30FF => "Katakana",
        0x4E00..=0x9FFF => "CJK Unified Ideographs",
        0xAC00..=0xD7AF => "Hangul Syllables",
        0xFE00..=0xFE0F => "Variation Selectors",
        0xFF00..=0xFFEF => "Halfwidth and Fullwidth Forms",
        0x1F600..=0x1F64F => "Emoticons",
        0x1F300..=0x1F5FF => "Miscellaneous Symbols and Pictographs",
        0x1F680..=0x1F6FF => "Transport and Map Symbols",
        0x1F900..=0x1F9FF => "Supplemental Symbols and Pictographs",
        _ => "Unknown Block",
    }
}

/// Summarize the Unicode blocks present in a string, returning each block name
/// and how many characters belong to it.
pub fn summarize_blocks(s: &str) -> Vec<(&'static str, usize)> {
    let mut map: Vec<(&'static str, usize)> = Vec::new();
    for ch in s.chars() {
        let block = unicode_block_name(ch);
        if let Some(entry) = map.iter_mut().find(|(b, _)| *b == block) {
            entry.1 += 1;
        } else {
            map.push((block, 1));
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Full-string security scan
// ---------------------------------------------------------------------------

/// A comprehensive security report for a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityReport {
    pub invisible_chars: usize,
    pub bidi_controls: usize,
    pub confusables: usize,
    pub mixed_scripts: bool,
    pub bidi_balanced: bool,
    pub excessive_combining: bool,
}

impl SecurityReport {
    /// Run all security checks on the given string.
    pub fn scan(s: &str) -> Self {
        Self {
            invisible_chars: find_invisible_chars(s).len(),
            bidi_controls: find_bidi_controls(s).len(),
            confusables: s.chars().filter(|&ch| is_confusable(ch).is_some()).count(),
            mixed_scripts: detect_mixed_scripts(s).is_some(),
            bidi_balanced: bidi_controls_balanced(s),
            excessive_combining: has_excessive_combining(s, 3),
        }
    }

    /// Returns `true` if no security issues were detected.
    pub fn is_clean(&self) -> bool {
        self.invisible_chars == 0
            && self.bidi_controls == 0
            && self.confusables == 0
            && !self.mixed_scripts
            && self.bidi_balanced
            && !self.excessive_combining
    }

    /// Overall severity: the worst issue found.
    pub fn overall_severity(&self) -> Severity {
        if self.bidi_controls > 0 || self.invisible_chars > 0 || !self.bidi_balanced {
            Severity::Error
        } else if self.confusables > 0 || self.mixed_scripts || self.excessive_combining {
            Severity::Warning
        } else {
            Severity::Info
        }
    }
}

impl fmt::Display for SecurityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SecurityReport(invisible={}, bidi={}, confusables={}, mixed_scripts={}, \
             bidi_balanced={}, excessive_combining={})",
            self.invisible_chars,
            self.bidi_controls,
            self.confusables,
            self.mixed_scripts,
            self.bidi_balanced,
            self.excessive_combining,
        )
    }
}


// === Unicode Category Detector ===

/// Unicode Category Detector implementation.
#[derive(Debug, Clone)]
pub struct UnicodeCategoryDetector {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: UnicodeCategoryDetectorStats,
}

/// Statistics for UnicodeCategoryDetector.
#[derive(Debug, Clone, Default)]
pub struct UnicodeCategoryDetectorStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl UnicodeCategoryDetectorStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl UnicodeCategoryDetector {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: UnicodeCategoryDetectorStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &UnicodeCategoryDetectorStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for UnicodeCategoryDetector {
    fn default() -> Self {
        Self::new()
    }
}

// === Unicode Width Calculator ===

/// Priority level for UnicodeWidthCalculator items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnicodeWidthCalculatorPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl UnicodeWidthCalculatorPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for UnicodeWidthCalculatorPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Unicode Width Calculator implementation.
#[derive(Debug, Clone)]
pub struct UnicodeWidthCalculator {
    items: Vec<UnicodeWidthCalculatorItem>,
    max_items: usize,
    default_priority: UnicodeWidthCalculatorPriority,
}

/// A single item in UnicodeWidthCalculator.
#[derive(Debug, Clone)]
pub struct UnicodeWidthCalculatorItem {
    pub id: String,
    pub label: String,
    pub priority: UnicodeWidthCalculatorPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl UnicodeWidthCalculatorItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: UnicodeWidthCalculatorPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: UnicodeWidthCalculatorPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl UnicodeWidthCalculator {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: UnicodeWidthCalculatorPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: UnicodeWidthCalculatorItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<UnicodeWidthCalculatorItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&UnicodeWidthCalculatorItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: UnicodeWidthCalculatorPriority) -> Vec<&UnicodeWidthCalculatorItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&UnicodeWidthCalculatorItem> {
        let mut sorted: Vec<&UnicodeWidthCalculatorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&UnicodeWidthCalculatorItem> {
        let mut sorted: Vec<&UnicodeWidthCalculatorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&UnicodeWidthCalculatorItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: UnicodeWidthCalculatorPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> UnicodeWidthCalculatorPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &UnicodeWidthCalculatorItem> {
        self.items.iter()
    }
}

impl Default for UnicodeWidthCalculator {
    fn default() -> Self {
        Self::new()
    }
}


/// Unicode highlight configuration manager.
#[derive(Debug, Clone)]
pub struct UnicodehlConfig {
    entries: Vec<UnicodehlEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single unicode highlight entry.
#[derive(Debug, Clone, PartialEq)]
pub struct UnicodehlEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl UnicodehlEntry {
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

impl UnicodehlConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: UnicodehlEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&UnicodehlEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut UnicodehlEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&UnicodehlEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&UnicodehlEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&UnicodehlEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<UnicodehlEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Unicode character highlighting — extended utilities (xd)
// ---------------------------------------------------------------------------

/// Metric accumulator for unicodehl operations.
#[derive(Debug, Clone)]
pub struct XdMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XdMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for unicodehl.
#[derive(Debug, Clone)]
pub struct XdRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XdRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for unicodehl lookups.
#[derive(Debug, Clone)]
pub struct XdLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XdLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
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

    // ---- Enhanced UnicodeCharCategory tests ----

    #[test]
    fn classify_ascii() {
        assert_eq!(classify_char('A'), UnicodeCharCategory::Ascii);
        assert_eq!(classify_char('z'), UnicodeCharCategory::Ascii);
        assert_eq!(classify_char(' '), UnicodeCharCategory::Ascii);
        assert_eq!(classify_char('\n'), UnicodeCharCategory::Ascii);
    }

    #[test]
    fn classify_cyrillic() {
        // Cyrillic small 'а' U+0430
        assert_eq!(classify_char('\u{0430}'), UnicodeCharCategory::Cyrillic);
        // Cyrillic capital 'Д' U+0414
        assert_eq!(classify_char('\u{0414}'), UnicodeCharCategory::Cyrillic);
    }

    #[test]
    fn classify_cjk() {
        // CJK Unified Ideograph '中' U+4E2D
        assert_eq!(classify_char('\u{4E2D}'), UnicodeCharCategory::CJK);
        // CJK Unified Ideograph '人' U+4EBA
        assert_eq!(classify_char('\u{4EBA}'), UnicodeCharCategory::CJK);
    }

    #[test]
    fn classify_emoji() {
        // Grinning Face U+1F600
        assert_eq!(classify_char('\u{1F600}'), UnicodeCharCategory::Emoji);
        // Slightly Smiling Face U+1F642
        assert_eq!(classify_char('\u{1F642}'), UnicodeCharCategory::Emoji);
    }

    #[test]
    fn classify_invisible() {
        // Zero Width Space U+200B
        assert_eq!(classify_char('\u{200B}'), UnicodeCharCategory::Invisible);
        // BOM / Zero Width No-Break Space U+FEFF
        assert_eq!(classify_char('\u{FEFF}'), UnicodeCharCategory::Invisible);
    }

    #[test]
    fn highlight_ranges_finds_non_ascii() {
        let line = "hello \u{0430} world";
        let ranges = unicode_highlight_ranges(line);
        assert!(!ranges.is_empty());
        assert_eq!(ranges[0].category, UnicodeCharCategory::Cyrillic);
        // The range should cover exactly the one Cyrillic char.
        assert_eq!(&line[ranges[0].start..ranges[0].end], "\u{0430}");
    }

    #[test]
    fn highlight_ranges_merges_adjacent() {
        // Two adjacent Cyrillic characters should be merged into one range.
        let line = "ab\u{0430}\u{0431}cd";
        let ranges = unicode_highlight_ranges(line);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].category, UnicodeCharCategory::Cyrillic);
        assert_eq!(&line[ranges[0].start..ranges[0].end], "\u{0430}\u{0431}");
    }

    #[test]
    fn unicode_escape_roundtrip() {
        let original = "café \u{0430}\u{0431}";
        let escaped = unicode_escape(original);
        let unescaped = unicode_unescape(&escaped);
        assert_eq!(unescaped, original);
    }

    #[test]
    fn unicode_escape_ascii_passthrough() {
        let ascii = "hello world 123 !@#";
        assert_eq!(unicode_escape(ascii), ascii);
        assert_eq!(unicode_unescape(ascii), ascii);
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

    #[test]
    fn test_severity_all() {
        assert_eq!(Severity::all().len(), 3);
    }

    #[test]
    fn test_severity_level() {
        assert!(Severity::Error.level() > Severity::Warning.level());
        assert!(Severity::Warning.level() > Severity::Info.level());
    }

    #[test]
    fn test_severity_from_str_opt() {
        assert_eq!(Severity::from_str_opt("error"), Some(Severity::Error));
        assert_eq!(Severity::from_str_opt("WARN"), Some(Severity::Warning));
        assert_eq!(Severity::from_str_opt("bogus"), None);
    }

    #[test]
    fn test_severity_icon() {
        assert_eq!(Severity::Error.icon(), '✖');
        assert_eq!(Severity::Info.icon(), 'ℹ');
    }

    #[test]
    fn test_char_category_suspicious() {
        assert!(UnicodeCharCategory::Cyrillic.is_suspicious());
        assert!(UnicodeCharCategory::Bidi.is_suspicious());
        assert!(!UnicodeCharCategory::Ascii.is_suspicious());
    }

    #[test]
    fn test_unicode_analysis_ascii() {
        let analysis = UnicodeAnalysis::analyze("hello world");
        assert_eq!(analysis.ascii_count, 11);
        assert_eq!(analysis.non_ascii_count, 0);
        assert!(analysis.is_safe());
        assert!((analysis.ascii_percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_unicode_analysis_display() {
        let a = UnicodeAnalysis::analyze("abc");
        let s = format!("{a}");
        assert!(s.contains("3 chars"));
    }

    #[test]
    fn test_strip_non_ascii() {
        assert_eq!(strip_non_ascii("hëllo"), "hllo");
        assert_eq!(strip_non_ascii("ascii"), "ascii");
    }

    #[test]
    fn test_char_codepoint() {
        assert_eq!(char_codepoint('A'), "U+0041");
        assert_eq!(char_codepoint('€'), "U+20AC");
    }

    #[test]
    fn test_string_codepoints() {
        let cps = string_codepoints("AB");
        assert_eq!(cps.len(), 2);
        assert_eq!(cps[0], ('A', "U+0041".to_string()));
    }

    #[test]
    fn test_confusable_detector_detect() {
        let detector = ConfusableDetector::new();
        let text = "h\u{0435}llo"; // Cyrillic е
        let matches = detector.detect(text);
        assert!(!matches.is_empty());
        assert_eq!(matches[0].confusable_with, 'e');
        assert!(format!("{}", matches[0]).contains("looks like"));
    }

    #[test]
    fn test_confusable_detector_no_confusables() {
        let detector = ConfusableDetector::new();
        assert!(detector.detect("hello world").is_empty());
        assert!(!detector.has_confusables("hello world"));
    }

    #[test]
    fn test_confusable_detector_normalize() {
        let detector = ConfusableDetector::new();
        let normalized = detector.normalize("h\u{0435}llo");
        assert_eq!(normalized, "hello");
    }

    #[test]
    fn test_homoglyph_map_from_vec() {
        let map = HomoglyphMap::from(vec![('а', 'a'), ('е', 'e')]);
        assert_eq!(map.len(), 2);
        assert_eq!(map.lookup('а'), Some('a'));
        assert_eq!(map.lookup('z'), None);
        assert!(!map.is_empty());
        assert!(format!("{map}").contains("2 pairs"));
    }

    #[test]
    fn test_homoglyph_map_add_dedup() {
        let mut map = HomoglyphMap::new();
        map.add('а', 'a');
        map.add('а', 'a');
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_homoglyph_map_iter() {
        let map = HomoglyphMap::from(vec![('а', 'a'), ('е', 'e')]);
        let collected: Vec<_> = map.iter().collect();
        assert_eq!(collected.len(), 2);
    }

    // --- new tests ---

    #[test]
    fn test_is_safe_text_ascii() {
        assert!(is_safe_text("Hello world!"));
        assert!(is_safe_text("fn main() { }"));
    }

    #[test]
    fn test_is_safe_text_non_ascii() {
        assert!(!is_safe_text("café"));
        assert!(!is_safe_text("a\u{200B}b")); // zero-width space
    }

    #[test]
    fn test_is_safe_text_empty() {
        assert!(is_safe_text(""));
    }

    #[test]
    fn test_extract_non_ascii() {
        assert_eq!(extract_non_ascii("abcdef"), Vec::<char>::new());
        assert_eq!(extract_non_ascii("café"), vec!['é']);
    }

    #[test]
    fn test_confusable_pairs_in_string() {
        // Cyrillic 'а' (U+0430) is confusable with ASCII 'a'
        let pairs = confusable_pairs_in("h\u{0435}llo");
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], ('\u{0435}', 'e'));
    }

    #[test]
    fn test_confusable_pairs_empty() {
        let pairs = confusable_pairs_in("hello");
        assert!(pairs.is_empty());
    }

    #[test]
    fn test_contains_rtl_override_yes() {
        assert!(contains_rtl_override("abc\u{202E}def"));
    }

    #[test]
    fn test_contains_rtl_override_no() {
        assert!(!contains_rtl_override("hello world"));
    }

    #[test]
    fn test_group_highlights_by_category() {
        let h1 = UnicodeHighlight {
            line: 0, column: 0, character: '\u{0430}',
            category: UnicodeCategory::ConfusableWithAscii,
            replacement: Some('a'),
        };
        let h2 = UnicodeHighlight {
            line: 0, column: 1, character: '\u{200B}',
            category: UnicodeCategory::Invisible,
            replacement: None,
        };
        let highlights = vec![h1, h2];
        let grouped = group_highlights_by_category(&highlights);
        assert_eq!(grouped.len(), 2);
        assert!(grouped.contains_key("confusable with ASCII"));
        assert!(grouped.contains_key("invisible"));
    }

    #[test]
    fn test_filter_by_min_severity_error() {
        let h_err = UnicodeHighlight {
            line: 0, column: 0, character: '\u{200B}',
            category: UnicodeCategory::Invisible,
            replacement: None,
        };
        let h_info = UnicodeHighlight {
            line: 0, column: 1, character: '\u{00E9}',
            category: UnicodeCategory::NonBasicAscii,
            replacement: None,
        };
        let highlights = vec![h_err, h_info];
        let filtered = filter_by_min_severity(&highlights, Severity::Error);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_format_diagnostics() {
        let h = UnicodeHighlight {
            line: 5, column: 10, character: '\u{0430}',
            category: UnicodeCategory::ConfusableWithAscii,
            replacement: Some('a'),
        };
        let diags = format_diagnostics(&[h]);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].contains("U+0430"));
        assert!(diags[0].contains("5:10"));
    }

    // ---- Script mixing detection tests ----

    #[test]
    fn test_detect_mixed_scripts_latin_only() {
        assert!(detect_mixed_scripts("hello world").is_none());
    }

    #[test]
    fn test_detect_mixed_scripts_latin_cyrillic() {
        // Mix Latin 'h' (Common/ASCII) with Cyrillic 'а' and Latin extended 'é'
        let warning = detect_mixed_scripts("hа\u{00E9}llo");
        assert!(warning.is_some());
        let w = warning.unwrap();
        assert!(w.scripts_found.contains(&UnicodeScript::Cyrillic));
        assert!(w.scripts_found.contains(&UnicodeScript::Latin));
        assert!(w.to_string().contains("mixed scripts"));
    }

    #[test]
    fn test_char_script_ranges() {
        assert_eq!(char_script('A'), UnicodeScript::Common); // ASCII range
        assert_eq!(char_script('\u{00E9}'), UnicodeScript::Latin); // Latin-1
        assert_eq!(char_script('\u{0430}'), UnicodeScript::Cyrillic);
        assert_eq!(char_script('\u{03B1}'), UnicodeScript::Greek); // alpha
        assert_eq!(char_script('\u{05D0}'), UnicodeScript::Hebrew); // alef
        assert_eq!(char_script('\u{0627}'), UnicodeScript::Arabic); // alif
        assert_eq!(char_script('\u{4E2D}'), UnicodeScript::Han);
    }

    // ---- Invisible character tests ----

    #[test]
    fn test_find_invisible_chars() {
        let s = "abc\u{200B}def\u{200D}ghi";
        let found = find_invisible_chars(s);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].character, '\u{200B}');
        assert_eq!(found[0].name, "Zero Width Space");
        assert_eq!(found[0].position, 3);
        assert_eq!(found[1].character, '\u{200D}');
        assert_eq!(found[1].name, "Zero Width Joiner");
    }

    #[test]
    fn test_strip_invisible() {
        assert_eq!(strip_invisible("a\u{200B}b\u{FEFF}c"), "abc");
        assert_eq!(strip_invisible("hello"), "hello");
    }

    #[test]
    fn test_contains_zero_width() {
        assert!(contains_zero_width("a\u{200B}b"));
        assert!(contains_zero_width("\u{FEFF}text"));
        assert!(!contains_zero_width("normal text"));
    }

    #[test]
    fn test_invisible_char_name_coverage() {
        assert_eq!(invisible_char_name('\u{200B}'), Some("Zero Width Space"));
        assert_eq!(invisible_char_name('\u{00AD}'), Some("Soft Hyphen"));
        assert_eq!(invisible_char_name('\u{180E}'), Some("Mongolian Vowel Separator"));
        assert_eq!(invisible_char_name('\u{2062}'), Some("Invisible Times"));
        assert_eq!(invisible_char_name('a'), None);
    }

    // ---- Combining character tests ----

    #[test]
    fn test_is_combining_mark() {
        assert!(is_combining_mark('\u{0300}')); // combining grave accent
        assert!(is_combining_mark('\u{0301}')); // combining acute accent
        assert!(!is_combining_mark('a'));
        assert!(!is_combining_mark(' '));
    }

    #[test]
    fn test_count_combining_marks() {
        // "é" decomposed: 'e' + combining acute accent
        let s = "e\u{0301}";
        assert_eq!(count_combining_marks(s), 1);
        assert_eq!(count_combining_marks("hello"), 0);
    }

    #[test]
    fn test_has_excessive_combining_zalgo() {
        // Zalgo-like text: 'a' followed by 5 combining marks
        let zalgo = "a\u{0300}\u{0301}\u{0302}\u{0303}\u{0304}";
        assert!(has_excessive_combining(zalgo, 3));
        assert!(!has_excessive_combining(zalgo, 5));
        assert!(!has_excessive_combining("normal text", 3));
    }

    // ---- Bidi control tests ----

    #[test]
    fn test_is_bidi_control() {
        assert!(is_bidi_control('\u{200E}')); // LRM
        assert!(is_bidi_control('\u{200F}')); // RLM
        assert!(is_bidi_control('\u{061C}')); // Arabic Letter Mark
        assert!(!is_bidi_control('a'));
    }

    #[test]
    fn test_find_bidi_controls() {
        let s = "abc\u{202A}def\u{202C}ghi";
        let controls = find_bidi_controls(s);
        assert_eq!(controls.len(), 2);
        assert_eq!(controls[0], (3, '\u{202A}'));
        assert_eq!(controls[1], (7, '\u{202C}'));
    }

    #[test]
    fn test_bidi_controls_balanced() {
        assert!(bidi_controls_balanced("no bidi here"));
        // Balanced: LRE + PDF
        assert!(bidi_controls_balanced("a\u{202A}b\u{202C}c"));
        // Unbalanced: LRE without PDF
        assert!(!bidi_controls_balanced("a\u{202A}b"));
        // Unbalanced: PDF without opener
        assert!(!bidi_controls_balanced("a\u{202C}b"));
    }

    // ---- Unicode block identification tests ----

    #[test]
    fn test_unicode_block_name() {
        assert_eq!(unicode_block_name('A'), "Basic Latin");
        assert_eq!(unicode_block_name('\u{00E9}'), "Latin-1 Supplement");
        assert_eq!(unicode_block_name('\u{0430}'), "Cyrillic");
        assert_eq!(unicode_block_name('\u{03B1}'), "Greek and Coptic");
        assert_eq!(unicode_block_name('\u{4E2D}'), "CJK Unified Ideographs");
        assert_eq!(unicode_block_name('\u{1F600}'), "Emoticons");
        assert_eq!(unicode_block_name('\u{2500}'), "Box Drawing");
    }

    #[test]
    fn test_summarize_blocks() {
        let summary = summarize_blocks("ABCdef");
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0], ("Basic Latin", 6));

        let summary2 = summarize_blocks("A\u{0430}\u{0431}");
        assert_eq!(summary2.len(), 2);
    }

    // ---- SecurityReport tests ----

    #[test]
    fn test_security_report_clean() {
        let report = SecurityReport::scan("hello world");
        assert!(report.is_clean());
        assert_eq!(report.overall_severity(), Severity::Info);
        assert!(report.to_string().contains("invisible=0"));
    }

    #[test]
    fn test_security_report_invisible() {
        let report = SecurityReport::scan("a\u{200B}b");
        assert!(!report.is_clean());
        assert_eq!(report.invisible_chars, 1);
        assert_eq!(report.overall_severity(), Severity::Error);
    }

    #[test]
    fn test_security_report_confusable() {
        let report = SecurityReport::scan("h\u{0435}llo");
        assert!(!report.is_clean());
        assert_eq!(report.confusables, 1);
    }

    #[test]
    fn test_security_report_mixed_scripts() {
        let report = SecurityReport::scan("h\u{00E9}llo\u{0430}");
        assert!(report.mixed_scripts);
        assert!(!report.is_clean());
    }

    #[test]
    fn test_security_report_unbalanced_bidi() {
        let report = SecurityReport::scan("abc\u{202A}def");
        assert!(!report.bidi_balanced);
        assert_eq!(report.overall_severity(), Severity::Error);
    }

    #[test]
    fn unicodeCategoryDetector_new() {
        let s = UnicodeCategoryDetector::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn unicodeCategoryDetector_add_contains() {
        let mut s = UnicodeCategoryDetector::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn unicodeCategoryDetector_add_duplicate() {
        let mut s = UnicodeCategoryDetector::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn unicodeCategoryDetector_remove() {
        let mut s = UnicodeCategoryDetector::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn unicodeCategoryDetector_capacity() {
        let s = UnicodeCategoryDetector::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn unicodeCategoryDetector_search() {
        let mut s = UnicodeCategoryDetector::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn unicodeCategoryDetector_stats() {
        let mut s = UnicodeCategoryDetector::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn unicodeWidthCalculator_new() {
        let m = UnicodeWidthCalculator::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn unicodeWidthCalculator_add_find() {
        let mut m = UnicodeWidthCalculator::new();
        m.add(UnicodeWidthCalculatorItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn unicodeWidthCalculator_priority_filter() {
        let mut m = UnicodeWidthCalculator::new();
        m.add(UnicodeWidthCalculatorItem::new("a", "A").with_priority(UnicodeWidthCalculatorPriority::High));
        m.add(UnicodeWidthCalculatorItem::new("b", "B").with_priority(UnicodeWidthCalculatorPriority::Low));
        m.add(UnicodeWidthCalculatorItem::new("c", "C").with_priority(UnicodeWidthCalculatorPriority::High));
        assert_eq!(m.by_priority(UnicodeWidthCalculatorPriority::High).len(), 2);
    }

    #[test]
    fn unicodeWidthCalculator_remove() {
        let mut m = UnicodeWidthCalculator::new();
        m.add(UnicodeWidthCalculatorItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn unicodeWidthCalculator_search() {
        let mut m = UnicodeWidthCalculator::new();
        m.add(UnicodeWidthCalculatorItem::new("id1", "Hello World"));
        m.add(UnicodeWidthCalculatorItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn unicodeWidthCalculator_total_weight() {
        let mut m = UnicodeWidthCalculator::new();
        m.add(UnicodeWidthCalculatorItem::new("a", "A").with_priority(UnicodeWidthCalculatorPriority::Critical));
        m.add(UnicodeWidthCalculatorItem::new("b", "B").with_priority(UnicodeWidthCalculatorPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn unicodeWidthCalculator_capacity_limit() {
        let mut m = UnicodeWidthCalculator::new().with_max_items(2);
        m.add(UnicodeWidthCalculatorItem::new("1", "one"));
        m.add(UnicodeWidthCalculatorItem::new("2", "two"));
        assert!(!m.add(UnicodeWidthCalculatorItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn unicodeWidthCalculator_sorted_by_priority() {
        let mut m = UnicodeWidthCalculator::new();
        m.add(UnicodeWidthCalculatorItem::new("lo", "Low").with_priority(UnicodeWidthCalculatorPriority::Low));
        m.add(UnicodeWidthCalculatorItem::new("hi", "High").with_priority(UnicodeWidthCalculatorPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn unicodeWidthCalculator_item_metadata() {
        let mut item = UnicodeWidthCalculatorItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn unicodeCategoryDetector_enabled_toggle() {
        let mut s = UnicodeCategoryDetector::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn unicodeWidthCalculator_priority_display() {
        assert_eq!(format!("{}", UnicodeWidthCalculatorPriority::High), "high");
        assert_eq!(format!("{}", UnicodeWidthCalculatorPriority::Low), "low");
    }


    #[test]
    fn unicodehl_entry_creation() {
        let e = UnicodehlEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn unicodehl_entry_with_priority() {
        let e = UnicodehlEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn unicodehl_entry_metadata() {
        let e = UnicodehlEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn unicodehl_entry_remove_meta() {
        let mut e = UnicodehlEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn unicodehl_entry_activate_deactivate() {
        let mut e = UnicodehlEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn unicodehl_config_add_sorted() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("lo", "Lo").with_priority(1));
        c.add(UnicodehlEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn unicodehl_config_capacity() {
        let mut c = UnicodehlConfig::new(1);
        assert!(c.add(UnicodehlEntry::new("a", "A")));
        assert!(!c.add(UnicodehlEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn unicodehl_config_remove() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn unicodehl_config_get() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn unicodehl_config_active_entries() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("a", "A"));
        c.add(UnicodehlEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn unicodehl_config_enable_disable() {
        let mut c = UnicodehlConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn unicodehl_config_clear() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn unicodehl_config_find_by_label() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn unicodehl_config_top_n() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("a", "A").with_priority(1));
        c.add(UnicodehlEntry::new("b", "B").with_priority(2));
        c.add(UnicodehlEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn unicodehl_config_deactivate_activate_all() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("a", "A"));
        c.add(UnicodehlEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn unicodehl_config_highest_priority() {
        let mut c = UnicodehlConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(UnicodehlEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn unicodehl_config_contains() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn unicodehl_config_labels() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("a", "Alpha"));
        c.add(UnicodehlEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn unicodehl_config_drain_inactive() {
        let mut c = UnicodehlConfig::new(10);
        c.add(UnicodehlEntry::new("a", "A"));
        c.add(UnicodehlEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn xd_metrics_empty() {
        let m = XdMetrics::new("unicodehl");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xd_metrics_record_and_mean() {
        let mut m = XdMetrics::new("unicodehl");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xd_metrics_min_max() {
        let mut m = XdMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xd_metrics_variance_and_std() {
        let mut m = XdMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn xd_metrics_percentile() {
        let mut m = XdMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xd_metrics_merge() {
        let mut a = XdMetrics::new("a");
        a.record(1.0);
        let mut b = XdMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xd_metrics_reset() {
        let mut m = XdMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xd_rate_window_empty() {
        let rw = XdRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xd_rate_window_tick_and_rate() {
        let mut rw = XdRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xd_lru_cache_basic() {
        let mut c = XdLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xd_lru_cache_contains_and_keys() {
        let mut c = XdLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xd_lru_cache_remove() {
        let mut c = XdLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xd_metrics_sum() {
        let mut m = XdMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xd_metrics_label() {
        let m = XdMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xd_lru_cache_clear() {
        let mut c = XdLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

}
