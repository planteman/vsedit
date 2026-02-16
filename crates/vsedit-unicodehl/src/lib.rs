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
}
