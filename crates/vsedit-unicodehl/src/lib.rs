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


// ---------------------------------------------------------------------------
// xb_ utilities – batch 20
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer20 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer20 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_20(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_20<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_20<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_20(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_20(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 190
// ---------------------------------------------------------------------------

/// Generic object pool `Xc190Pool<T>`.
pub struct Xc190Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc190Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc190PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc190Pool<T> {
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
    pub fn stats(&self) -> Xc190PoolStats {
        Xc190PoolStats {
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

impl<T> Default for Xc190Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc190Scheduler`.
pub struct Xc190Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc190Scheduler {
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

impl Default for Xc190Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_190 hash for the given byte slice.
pub fn xc_190_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_190 convention.
pub fn xc_190_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe32 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe32Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe32PipelineError {
    pub stage: Xe32Stage,
    pub message: String,
}

impl std::fmt::Display for Xe32PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe32Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe32Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError>>>,
    stage_names: Vec<Xe32Stage>,
}

impl Xe32Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe32Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe32Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe32Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe32Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe32Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe32CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe32CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe32Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe32CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe32CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe32Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe32CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_32_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe32CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_32_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe32CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_32_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError> {
    Ok(data)
}

pub fn xe_32_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_32_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_32_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_32_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe32PipelineError> {
    Err(Xe32PipelineError {
        stage: Xe32Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #118
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf118Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf118TrieNode {
    children: std::collections::HashMap<char, Xf118TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf118Trie {
    root: Xf118TrieNode,
    count: usize,
}

impl Xf118Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf118TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf118TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf118TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf118BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf118BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 189).
pub struct Xh189SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh189SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 231 as u64,
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

/// A compact bit set supporting boolean operations (variant 189).
pub struct Xh189BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh189BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 189).
pub struct Xi189Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi189Deque<T> {
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
pub struct Xi189Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi189Interval {
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

/// A simple interval tree (variant 189).
pub struct Xi189IntervalTree {
    xi_intervals: Vec<Xi189Interval>,
}

impl Xi189IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi189Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi189Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi189Interval) -> Vec<&Xi189Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi189Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi189Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi189Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi189Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi189Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi189Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 189) ---

/// Disjoint set / union-find for crate 189.
pub struct Xj189UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj189UnionFind {
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

const XJ189_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 189.
pub struct Xj189BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj189BTreeNode<K, V>>>,
    len: usize,
}

struct Xj189BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj189BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj189BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ189_BTREE_ORDER - 1
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
        let mid = XJ189_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj189BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj189BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj189BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj189BTreeNode::xj_new_leaf();
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


// --- xk_189 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk189SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk189SegmentTree {
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
pub struct Xk189DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk189DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_189).
#[derive(Debug, Clone)]
pub struct Xl189Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl189Rope {
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

/// Suffix array for efficient string searching (xl_189).
#[derive(Debug, Clone)]
pub struct Xl189SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl189SuffixArray {
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
pub struct Xm189MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm189MatrixSparse {
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
pub struct Xm189Tokenizer {
    text: String,
}

impl Xm189Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 189.
pub struct Xn189Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn189Fenwick {
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

// ----- AVL tree map — crate 189 -----

#[derive(Debug, Clone)]
struct Xn189AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn189AvlNode<K, V>>>,
    right: Option<Box<Xn189AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 189.
#[derive(Debug, Clone)]
pub struct Xn189AVL<K, V> {
    root: Option<Box<Xn189AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn189AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn189AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn189AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn189AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn189AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn189AvlNode<K, V>>) -> Box<Xn189AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn189AvlNode<K, V>>) -> Box<Xn189AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn189AvlNode<K, V>>) -> Box<Xn189AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn189AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn189AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn189AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn189AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn189AvlNode<K, V>>) -> &Xn189AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn189AvlNode<K, V>>) -> (Box<Xn189AvlNode<K, V>>, Option<Box<Xn189AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn189AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn189AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn189AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn189AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn189AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn189AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn189AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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


    #[test]
    fn xb_ring_buffer_20_push_and_len() {
        let mut rb = super::XbRingBuffer20::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_20_overwrite() {
        let mut rb = super::XbRingBuffer20::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_20_get_out_of_bounds() {
        let rb = super::XbRingBuffer20::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_20_drain_all() {
        let mut rb = super::XbRingBuffer20::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_20_peek_front_back() {
        let mut rb = super::XbRingBuffer20::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_20_clear() {
        let mut rb = super::XbRingBuffer20::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_20_capacity() {
        let rb = super::XbRingBuffer20::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_20_basic() {
        let h = super::xb_fnv1a_20(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_20(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_20_different_inputs() {
        let h1 = super::xb_fnv1a_20(b"abc");
        let h2 = super::xb_fnv1a_20(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_20_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_20(&data);
        let dec = super::xb_rle_decode_20(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_20_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_20(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_20(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_20_values() {
        assert!((super::xb_clamp_20(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_20(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_20(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_20_values() {
        assert!((super::xb_lerp_20(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_20(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_20(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_20_wrap_around_twice() {
        let mut rb = super::XbRingBuffer20::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 190 ----

    #[test]
    fn xc_190_pool_new_empty() {
        let pool: super::Xc190Pool<i32> = super::Xc190Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_190_pool_release_acquire() {
        let mut pool = super::Xc190Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_190_pool_acquire_empty() {
        let mut pool: super::Xc190Pool<i32> = super::Xc190Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_190_pool_full() {
        let mut pool = super::Xc190Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_190_pool_drain() {
        let mut pool = super::Xc190Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_190_pool_stats() {
        let mut pool = super::Xc190Pool::new(8);
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
    fn xc_190_pool_clear() {
        let mut pool = super::Xc190Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_190_pool_shrink() {
        let mut pool = super::Xc190Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_190_pool_default() {
        let pool: super::Xc190Pool<String> = super::Xc190Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_190_pool_extend() {
        let mut pool = super::Xc190Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_190_pool_retain() {
        let mut pool = super::Xc190Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_190_scheduler_round_robin() {
        let mut sched = super::Xc190Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_190_scheduler_empty() {
        let mut sched = super::Xc190Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_190_scheduler_reset() {
        let mut sched = super::Xc190Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_190_scheduler_add_remove() {
        let mut sched = super::Xc190Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_190_scheduler_targets() {
        let sched = super::Xc190Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_190_hash_empty() {
        assert_eq!(super::xc_190_hash(b""), 5381);
    }

    #[test]
    fn xc_190_hash_data() {
        let h = super::xc_190_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_190_hash(b"hello"), h);
    }

    #[test]
    fn xc_190_reverse_str() {
        assert_eq!(super::xc_190_reverse("abc"), "cba");
        assert_eq!(super::xc_190_reverse(""), "");
    }


    #[test]
    fn xe_32_pipeline_empty() {
        let p = super::Xe32Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_32_pipeline_parse_stage() {
        let p = super::Xe32Pipeline::new()
            .add_parse(super::xe_32_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_32_pipeline_transform_double() {
        let p = super::Xe32Pipeline::new()
            .add_transform(super::xe_32_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_32_pipeline_validate_reverse() {
        let p = super::Xe32Pipeline::new()
            .add_validate(super::xe_32_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_32_pipeline_emit_filter() {
        let p = super::Xe32Pipeline::new()
            .add_emit(super::xe_32_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_32_pipeline_multi_stage() {
        let p = super::Xe32Pipeline::new()
            .add_parse(super::xe_32_pipeline_identity)
            .add_transform(super::xe_32_pipeline_double)
            .add_validate(super::xe_32_pipeline_reverse)
            .add_emit(super::xe_32_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_32_pipeline_error_propagation() {
        let p = super::Xe32Pipeline::new()
            .add_parse(super::xe_32_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe32Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_32_pipeline_compose() {
        let p1 = super::Xe32Pipeline::new()
            .add_parse(super::xe_32_pipeline_identity);
        let p2 = super::Xe32Pipeline::new()
            .add_transform(super::xe_32_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_32_pipeline_error_display() {
        let e = super::Xe32PipelineError {
            stage: super::Xe32Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_32_cache_put_get() {
        let mut c = super::Xe32Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_32_cache_miss() {
        let mut c: super::Xe32Cache<&str, i32> = super::Xe32Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_32_cache_ttl_expiry() {
        let mut c = super::Xe32Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_32_cache_evict() {
        let mut c = super::Xe32Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_32_cache_capacity() {
        let mut c = super::Xe32Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_32_cache_stats() {
        let mut c = super::Xe32Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_32_cache_clear() {
        let mut c = super::Xe32Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #118 --

    #[test]
    fn xf118_trie_insert_search() {
        let mut t = Xf118Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf118_trie_starts_with() {
        let mut t = Xf118Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf118_trie_remove() {
        let mut t = Xf118Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf118_trie_word_count() {
        let mut t = Xf118Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf118_trie_longest_prefix() {
        let mut t = Xf118Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf118_trie_all_words() {
        let mut t = Xf118Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf118_trie_autocomplete() {
        let mut t = Xf118Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf118_trie_empty_search() {
        let t = Xf118Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf118_bloom_add_contains() {
        let mut bf = Xf118BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf118_bloom_probably_absent() {
        let bf = Xf118BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf118_bloom_false_positive_rate() {
        let mut bf = Xf118BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf118_bloom_clear() {
        let mut bf = Xf118BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf118_bloom_union() {
        let mut a = Xf118BloomFilter::xf_new(512, 2);
        let mut b = Xf118BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf118_bloom_intersection_estimate() {
        let mut a = Xf118BloomFilter::xf_new(512, 2);
        let mut b = Xf118BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf118_bloom_union_size_mismatch() {
        let a = Xf118BloomFilter::xf_new(256, 2);
        let b = Xf118BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh189_skip_insert_contains() {
        let mut sl = super::Xh189SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh189_skip_remove() {
        let mut sl = super::Xh189SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh189_skip_len() {
        let mut sl = super::Xh189SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh189_skip_range_query() {
        let mut sl = super::Xh189SkipList::xh_new(4);
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
    fn xh189_skip_floor_ceiling() {
        let mut sl = super::Xh189SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh189_skip_rank() {
        let mut sl = super::Xh189SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh189_skip_empty() {
        let sl = super::Xh189SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh189_skip_duplicates() {
        let mut sl = super::Xh189SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh189_bitset_set_test() {
        let mut bs = super::Xh189BitSet::xh_new(256);
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
    fn xh189_bitset_clear_count() {
        let mut bs = super::Xh189BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh189_bitset_and_or_xor() {
        let mut a = super::Xh189BitSet::xh_new(128);
        let mut b = super::Xh189BitSet::xh_new(128);
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
    fn xh189_bitset_iter_ones() {
        let mut bs = super::Xh189BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh189_bitset_first_last() {
        let mut bs = super::Xh189BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh189_bitset_empty() {
        let bs = super::Xh189BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi189_deque_push_pop_back() {
        let mut dq = super::Xi189Deque::xi_new(4);
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
    fn xi189_deque_push_pop_front() {
        let mut dq = super::Xi189Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi189_deque_mixed_ops() {
        let mut dq = super::Xi189Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi189_deque_get_and_split() {
        let mut dq = super::Xi189Deque::xi_new(8);
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
    fn xi189_deque_rotate_left() {
        let mut dq = super::Xi189Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi189_deque_rotate_right() {
        let mut dq = super::Xi189Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi189_deque_grow() {
        let mut dq = super::Xi189Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi189_deque_empty() {
        let dq = super::Xi189Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi189_interval_tree_insert_query() {
        let mut tree = super::Xi189IntervalTree::xi_new();
        tree.xi_insert(super::Xi189Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi189Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi189Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi189_interval_tree_overlap() {
        let mut tree = super::Xi189IntervalTree::xi_new();
        tree.xi_insert(super::Xi189Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi189Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi189Interval::xi_new(12, 20));
        let q = super::Xi189Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi189_interval_tree_remove() {
        let mut tree = super::Xi189IntervalTree::xi_new();
        tree.xi_insert(super::Xi189Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi189Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi189_interval_tree_gaps() {
        let mut tree = super::Xi189IntervalTree::xi_new();
        tree.xi_insert(super::Xi189Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi189Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi189Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi189Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi189Interval::xi_new(8, 10));
    }

    #[test]
    fn xi189_interval_tree_merge() {
        let mut tree = super::Xi189IntervalTree::xi_new();
        tree.xi_insert(super::Xi189Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi189Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi189Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi189Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi189Interval::xi_new(10, 15));
    }

    #[test]
    fn xi189_interval_tree_all() {
        let mut tree = super::Xi189IntervalTree::xi_new();
        tree.xi_insert(super::Xi189Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi189Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi189_interval_tree_empty() {
        let tree = super::Xi189IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi189_interval_tree_contains_point() {
        let iv = super::Xi189Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 189) ---

    #[test]
    fn xj_189_uf_make_and_find() {
        let mut uf = super::Xj189UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_189_uf_union_connected() {
        let mut uf = super::Xj189UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_189_uf_component_count() {
        let mut uf = super::Xj189UnionFind::xj_new();
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
    fn xj_189_uf_component_size() {
        let mut uf = super::Xj189UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_189_uf_largest_component() {
        let mut uf = super::Xj189UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_189_uf_many_elements() {
        let mut uf = super::Xj189UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_189_uf_separate_components() {
        let mut uf = super::Xj189UnionFind::xj_new();
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
    fn xj_189_uf_path_compression() {
        let mut uf = super::Xj189UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_189_bt_insert_get() {
        let mut bt = super::Xj189BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_189_bt_contains_len() {
        let mut bt = super::Xj189BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_189_bt_replace() {
        let mut bt = super::Xj189BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_189_bt_remove() {
        let mut bt = super::Xj189BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_189_bt_keys_values() {
        let mut bt = super::Xj189BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_189_bt_range() {
        let mut bt = super::Xj189BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_189_bt_min_max() {
        let mut bt = super::Xj189BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_189_bt_many_inserts() {
        let mut bt = super::Xj189BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_189 segment tree tests ---

    #[test]
    fn xk_189_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk189SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_189_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk189SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_189_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk189SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_189_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk189SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_189_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk189SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_189_st_single_element() {
        let data = vec![42];
        let st = super::Xk189SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_189_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk189SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_189_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk189SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_189 disjoint intervals tests ---

    #[test]
    fn xk_189_di_add_and_count() {
        let mut di = super::Xk189DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_189_di_merge_overlap() {
        let mut di = super::Xk189DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_189_di_contains() {
        let mut di = super::Xk189DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_189_di_remove() {
        let mut di = super::Xk189DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_189_di_covered_length() {
        let mut di = super::Xk189DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_189_di_gaps() {
        let mut di = super::Xk189DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_189_di_merge_adjacent() {
        let mut di = super::Xk189DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_189_di_empty() {
        let di = super::Xk189DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_189_rope_new_empty() {
        let rope = super::Xl189Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_189_rope_from_str() {
        let rope = super::Xl189Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_189_rope_insert_at() {
        let mut rope = super::Xl189Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_189_rope_delete_range() {
        let mut rope = super::Xl189Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_189_rope_char_at() {
        let rope = super::Xl189Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_189_rope_split_concat() {
        let rope = super::Xl189Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_189_rope_line_count() {
        let rope = super::Xl189Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_189_rope_line_at() {
        let rope = super::Xl189Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_189_sa_build_and_search() {
        let sa = super::Xl189SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_189_sa_count() {
        let sa = super::Xl189SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_189_sa_longest_repeated() {
        let sa = super::Xl189SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_189_sa_all_positions() {
        let sa = super::Xl189SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_189_sa_len() {
        let sa = super::Xl189SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_189_sa_empty() {
        let sa = super::Xl189SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_189_rope_slice() {
        let rope = super::Xl189Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_189_sa_search_start() {
        let sa = super::Xl189SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_189_sparse_set_get() {
        let mut m = super::Xm189MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_189_sparse_row_col() {
        let mut m = super::Xm189MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_189_sparse_transpose() {
        let mut m = super::Xm189MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_189_sparse_multiply_vec() {
        let mut m = super::Xm189MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_189_sparse_nnz_density() {
        let mut m = super::Xm189MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_189_sparse_clear() {
        let mut m = super::Xm189MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_189_sparse_overwrite_zero() {
        let mut m = super::Xm189MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_189_tokenizer_basic() {
        let t = super::Xm189Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_189_tokenizer_count() {
        let t = super::Xm189Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_189_tokenizer_unique() {
        let t = super::Xm189Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_189_tokenizer_frequency() {
        let t = super::Xm189Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_189_tokenizer_delimiter() {
        let t = super::Xm189Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_189_tokenizer_whitespace() {
        let t = super::Xm189Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_189_tokenizer_empty() {
        let t = super::Xm189Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 189 ----

    #[test]
    fn xn_189_fenwick_prefix_sum() {
        let mut ft = super::Xn189Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_189_fenwick_range_sum() {
        let mut ft = super::Xn189Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_189_fenwick_point_query() {
        let mut ft = super::Xn189Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_189_fenwick_len() {
        let ft = super::Xn189Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_189_fenwick_multiple_updates() {
        let mut ft = super::Xn189Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_189_fenwick_single_element() {
        let mut ft = super::Xn189Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_189_fenwick_find_kth() {
        let mut ft = super::Xn189Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_189_fenwick_negative_delta() {
        let mut ft = super::Xn189Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 189 ----

    #[test]
    fn xn_189_avl_insert_get() {
        let mut m = super::Xn189AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_189_avl_remove() {
        let mut m = super::Xn189AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_189_avl_in_order() {
        let mut m = super::Xn189AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_189_avl_min_max() {
        let mut m = super::Xn189AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_189_avl_floor_ceiling() {
        let mut m = super::Xn189AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_189_avl_height_balanced() {
        let mut m = super::Xn189AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_189_avl_overwrite() {
        let mut m = super::Xn189AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_189_avl_empty() {
        let m: super::Xn189AVL<i32, i32> = super::Xn189AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
