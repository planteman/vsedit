//! Tokenization infrastructure.
//!
//! Equivalent to VS Code's `vs/editor/common/languages/supports/tokenization.ts`.
//! Provides token types and tokenization results used by syntax highlighting.

use std::fmt;
/// Standard token type matching VS Code's classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardTokenType {
    Other = 0,
    Comment = 1,
    String = 2,
    RegExp = 3,
}

/// Token metadata bits (packed into u32 like VS Code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenMetadata(pub u32);

impl TokenMetadata {
    pub fn new(
        language_id: u8,
        token_type: StandardTokenType,
        font_style: FontStyle,
        foreground: u16,
        background: u16,
    ) -> Self {
        let value = (language_id as u32)
            | ((token_type as u32) << 8)
            | ((font_style.bits() as u32) << 11)
            | ((foreground as u32) << 15)
            | ((background as u32) << 24);
        Self(value)
    }

    pub fn language_id(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    pub fn token_type(&self) -> StandardTokenType {
        match (self.0 >> 8) & 0x7 {
            1 => StandardTokenType::Comment,
            2 => StandardTokenType::String,
            3 => StandardTokenType::RegExp,
            _ => StandardTokenType::Other,
        }
    }

    pub fn font_style(&self) -> FontStyle {
        FontStyle::from_bits_truncate(((self.0 >> 11) & 0xF) as u8)
    }

    pub fn foreground(&self) -> u16 {
        ((self.0 >> 15) & 0x1FF) as u16
    }

    pub fn background(&self) -> u16 {
        ((self.0 >> 24) & 0xFF) as u16
    }
}

/// Font style flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontStyle(u8);

impl FontStyle {
    pub const NONE: Self = Self(0);
    pub const ITALIC: Self = Self(1);
    pub const BOLD: Self = Self(2);
    pub const UNDERLINE: Self = Self(4);
    pub const STRIKETHROUGH: Self = Self(8);

    pub fn bits(&self) -> u8 {
        self.0
    }

    pub fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & 0xF)
    }

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn is_italic(&self) -> bool {
        self.contains(Self::ITALIC)
    }

    pub fn is_bold(&self) -> bool {
        self.contains(Self::BOLD)
    }

    pub fn is_underline(&self) -> bool {
        self.contains(Self::UNDERLINE)
    }
}

impl std::ops::BitOr for FontStyle {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// A token: start offset and metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub start_offset: u32,
    pub metadata: TokenMetadata,
}

/// Result of tokenizing a line.
#[derive(Debug, Clone, PartialEq)]
pub struct LineTokens {
    pub tokens: Vec<Token>,
}

impl LineTokens {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens }
    }

    pub fn empty() -> Self {
        Self {
            tokens: Vec::new(),
        }
    }

    /// Get the token at a given offset.
    pub fn token_at(&self, offset: u32) -> Option<&Token> {
        let idx = self
            .tokens
            .partition_point(|t| t.start_offset <= offset)
            .saturating_sub(1);
        self.tokens.get(idx)
    }

    pub fn count(&self) -> usize {
        self.tokens.len()
    }
}

/// Trait for tokenization providers.
pub trait ITokenizationSupport: Send + Sync {
    /// Tokenize a line given an initial state, returning tokens and next state.
    fn tokenize(&self, line: &str, state: TokenizationState) -> (LineTokens, TokenizationState);

    /// Get the initial state.
    fn get_initial_state(&self) -> TokenizationState;
}

/// Opaque tokenization state (carried between lines).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenizationState(pub u64);

impl TokenizationState {
    pub fn initial() -> Self {
        Self(0)
    }
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during token operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    /// A foreground color index exceeded the 9-bit maximum (0..=511).
    ForegroundOutOfRange(u16),
    /// A background color index exceeded the 8-bit maximum (0..=255).
    BackgroundOutOfRange(u16),
    /// Token offsets in a line are not monotonically non-decreasing.
    OffsetsNotSorted,
    /// Attempted to access a token index that does not exist.
    IndexOutOfBounds { index: usize, len: usize },
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForegroundOutOfRange(v) => {
                write!(f, "foreground color index {v} exceeds 9-bit max (511)")
            }
            Self::BackgroundOutOfRange(v) => {
                write!(f, "background color index {v} exceeds 8-bit max (255)")
            }
            Self::OffsetsNotSorted => write!(f, "token offsets are not sorted"),
            Self::IndexOutOfBounds { index, len } => {
                write!(f, "token index {index} out of bounds (len {len})")
            }
        }
    }
}

impl std::error::Error for TokenError {}

// ---------------------------------------------------------------------------
// Display implementations
// ---------------------------------------------------------------------------

impl std::fmt::Display for StandardTokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Other => write!(f, "Other"),
            Self::Comment => write!(f, "Comment"),
            Self::String => write!(f, "String"),
            Self::RegExp => write!(f, "RegExp"),
        }
    }
}

impl std::fmt::Display for FontStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == 0 {
            return write!(f, "None");
        }
        let mut parts = Vec::new();
        if self.is_italic() {
            parts.push("Italic");
        }
        if self.is_bold() {
            parts.push("Bold");
        }
        if self.is_underline() {
            parts.push("Underline");
        }
        if self.is_strikethrough() {
            parts.push("Strikethrough");
        }
        write!(f, "{}", parts.join("|"))
    }
}

impl std::fmt::Display for TokenMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "lang={} type={} style={} fg={} bg={}",
            self.language_id(),
            self.token_type(),
            self.font_style(),
            self.foreground(),
            self.background(),
        )
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}: {}", self.start_offset, self.metadata)
    }
}

// ---------------------------------------------------------------------------
// FontStyle helpers
// ---------------------------------------------------------------------------

impl FontStyle {
    /// Returns `true` if the strikethrough flag is set.
    pub fn is_strikethrough(&self) -> bool {
        self.contains(Self::STRIKETHROUGH)
    }

    /// Returns a new `FontStyle` with the given flag toggled.
    pub fn toggle(self, flag: Self) -> Self {
        Self(self.0 ^ flag.0)
    }

    /// Removes the specified flag.
    pub fn remove(self, flag: Self) -> Self {
        Self(self.0 & !flag.0)
    }

    /// Returns `true` when no style flags are set.
    pub fn is_none(&self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitAnd for FontStyle {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

// ---------------------------------------------------------------------------
// TokenMetadata – validated constructor & builder
// ---------------------------------------------------------------------------

impl TokenMetadata {
    /// Creates `TokenMetadata` with range-validated color indices.
    pub fn try_new(
        language_id: u8,
        token_type: StandardTokenType,
        font_style: FontStyle,
        foreground: u16,
        background: u16,
    ) -> Result<Self, TokenError> {
        if foreground > 0x1FF {
            return Err(TokenError::ForegroundOutOfRange(foreground));
        }
        if background > 0xFF {
            return Err(TokenError::BackgroundOutOfRange(background));
        }
        Ok(Self::new(language_id, token_type, font_style, foreground, background))
    }

    /// Returns a copy with only the `font_style` changed.
    pub fn with_font_style(&self, font_style: FontStyle) -> Self {
        Self::new(
            self.language_id(),
            self.token_type(),
            font_style,
            self.foreground(),
            self.background(),
        )
    }

    /// Returns a copy with only the `foreground` changed.
    pub fn with_foreground(&self, foreground: u16) -> Self {
        Self::new(
            self.language_id(),
            self.token_type(),
            self.font_style(),
            foreground,
            self.background(),
        )
    }

    /// Returns a copy with only the `background` changed.
    pub fn with_background(&self, background: u16) -> Self {
        Self::new(
            self.language_id(),
            self.token_type(),
            self.font_style(),
            self.foreground(),
            background,
        )
    }
}

// ---------------------------------------------------------------------------
// TokenMetadataBuilder
// ---------------------------------------------------------------------------

/// Builder for constructing `TokenMetadata` incrementally.
#[derive(Debug, Clone)]
pub struct TokenMetadataBuilder {
    language_id: u8,
    token_type: StandardTokenType,
    font_style: FontStyle,
    foreground: u16,
    background: u16,
}

impl Default for TokenMetadataBuilder {
    fn default() -> Self {
        Self {
            language_id: 0,
            token_type: StandardTokenType::Other,
            font_style: FontStyle::NONE,
            foreground: 0,
            background: 0,
        }
    }
}

impl TokenMetadataBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn language_id(mut self, id: u8) -> Self {
        self.language_id = id;
        self
    }

    pub fn token_type(mut self, tt: StandardTokenType) -> Self {
        self.token_type = tt;
        self
    }

    pub fn font_style(mut self, fs: FontStyle) -> Self {
        self.font_style = fs;
        self
    }

    pub fn foreground(mut self, fg: u16) -> Self {
        self.foreground = fg;
        self
    }

    pub fn background(mut self, bg: u16) -> Self {
        self.background = bg;
        self
    }

    /// Build the metadata, returning an error if color indices are out of range.
    pub fn build(self) -> Result<TokenMetadata, TokenError> {
        TokenMetadata::try_new(
            self.language_id,
            self.token_type,
            self.font_style,
            self.foreground,
            self.background,
        )
    }
}

// ---------------------------------------------------------------------------
// LineTokens – additional business-logic methods
// ---------------------------------------------------------------------------

impl LineTokens {
    /// Validates that token offsets are sorted in non-decreasing order.
    pub fn validate(&self) -> Result<(), TokenError> {
        for w in self.tokens.windows(2) {
            if w[1].start_offset < w[0].start_offset {
                return Err(TokenError::OffsetsNotSorted);
            }
        }
        Ok(())
    }

    /// Returns the token at the given positional index, or an error.
    pub fn get(&self, index: usize) -> Result<&Token, TokenError> {
        self.tokens.get(index).ok_or(TokenError::IndexOutOfBounds {
            index,
            len: self.tokens.len(),
        })
    }

    /// Returns the byte range `[start, end)` covered by the token at `index`.
    /// `line_len` is the total length of the line (used for the last token).
    pub fn token_range(&self, index: usize, line_len: u32) -> Result<(u32, u32), TokenError> {
        let tok = self.get(index)?;
        let end = self
            .tokens
            .get(index + 1)
            .map(|t| t.start_offset)
            .unwrap_or(line_len);
        Ok((tok.start_offset, end))
    }

    /// Returns an iterator over `(start_offset, end_offset, &TokenMetadata)`.
    pub fn iter_ranges(&self, line_len: u32) -> impl Iterator<Item = (u32, u32, &TokenMetadata)> {
        let tokens = &self.tokens;
        tokens.iter().enumerate().map(move |(i, tok)| {
            let end = tokens
                .get(i + 1)
                .map(|t| t.start_offset)
                .unwrap_or(line_len);
            (tok.start_offset, end, &tok.metadata)
        })
    }

    /// Merges two sorted `LineTokens` sequences by start offset.
    pub fn merge(&self, other: &LineTokens) -> LineTokens {
        let mut merged = Vec::with_capacity(self.tokens.len() + other.tokens.len());
        let (mut i, mut j) = (0, 0);
        while i < self.tokens.len() && j < other.tokens.len() {
            if self.tokens[i].start_offset <= other.tokens[j].start_offset {
                merged.push(self.tokens[i]);
                i += 1;
            } else {
                merged.push(other.tokens[j]);
                j += 1;
            }
        }
        merged.extend_from_slice(&self.tokens[i..]);
        merged.extend_from_slice(&other.tokens[j..]);
        LineTokens::new(merged)
    }

    /// Returns `true` if any token has the given `StandardTokenType`.
    pub fn contains_type(&self, tt: StandardTokenType) -> bool {
        self.tokens.iter().any(|t| t.metadata.token_type() == tt)
    }

    /// Returns the number of tokens whose type matches `tt`.
    pub fn count_type(&self, tt: StandardTokenType) -> usize {
        self.tokens
            .iter()
            .filter(|t| t.metadata.token_type() == tt)
            .count()
    }
}

// ---------------------------------------------------------------------------
// TokenizationState helpers
// ---------------------------------------------------------------------------

impl std::fmt::Display for TokenizationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "State({})", self.0)
    }
}

impl TokenizationState {
    /// Returns `true` if this is the initial (zero) state.
    pub fn is_initial(&self) -> bool {
        self.0 == 0
    }
}

/// Statistics about tokens in a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenStatistics {
    pub total_tokens: usize,
    pub comment_count: usize,
    pub string_count: usize,
    pub regexp_count: usize,
    pub other_count: usize,
}

/// Computes token type statistics for a `LineTokens`.
pub fn compute_token_statistics(line_tokens: &LineTokens) -> TokenStatistics {
    let mut comment_count = 0;
    let mut string_count = 0;
    let mut regexp_count = 0;
    let mut other_count = 0;
    for t in &line_tokens.tokens {
        match t.metadata.token_type() {
            StandardTokenType::Comment => comment_count += 1,
            StandardTokenType::String => string_count += 1,
            StandardTokenType::RegExp => regexp_count += 1,
            StandardTokenType::Other => other_count += 1,
        }
    }
    TokenStatistics {
        total_tokens: line_tokens.count(),
        comment_count,
        string_count,
        regexp_count,
        other_count,
    }
}

/// Merges overlapping or adjacent token ranges, keeping the first token's metadata.
pub fn merge_token_ranges(tokens: &[Token]) -> Vec<Token> {
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut result = vec![tokens[0]];
    for t in &tokens[1..] {
        let last = result.last().unwrap();
        if t.start_offset == last.start_offset {
            continue;
        }
        result.push(*t);
    }
    result
}

/// Maps a `StandardTokenType` to a color name string.
pub fn token_type_color_name(tt: StandardTokenType) -> &'static str {
    match tt {
        StandardTokenType::Comment => "comment.foreground",
        StandardTokenType::String => "string.foreground",
        StandardTokenType::RegExp => "regexp.foreground",
        StandardTokenType::Other => "editor.foreground",
    }
}

/// A simple tokenization cache keyed by line number.
#[derive(Debug, Clone, Default)]
pub struct TokenizationCache {
    entries: Vec<Option<(LineTokens, TokenizationState)>>,
}

impl TokenizationCache {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn get(&self, line: usize) -> Option<&(LineTokens, TokenizationState)> {
        self.entries.get(line).and_then(|e| e.as_ref())
    }

    pub fn set(&mut self, line: usize, tokens: LineTokens, state: TokenizationState) {
        if line >= self.entries.len() {
            self.entries.resize_with(line + 1, || None);
        }
        self.entries[line] = Some((tokens, state));
    }

    pub fn invalidate(&mut self, from_line: usize) {
        for i in from_line..self.entries.len() {
            self.entries[i] = None;
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn cached_line_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    pub fn capacity(&self) -> usize {
        self.entries.len()
    }
}

/// Returns a human-readable name for a `StandardTokenType` variant.
pub fn token_type_name(tt: StandardTokenType) -> &'static str {
    match tt {
        StandardTokenType::Other => "Other",
        StandardTokenType::Comment => "Comment",
        StandardTokenType::String => "String",
        StandardTokenType::RegExp => "RegExp",
    }
}

// ---------------------------------------------------------------------------
// Token classification and enhanced statistics
// ---------------------------------------------------------------------------

/// Fine-grained token classification beyond the basic `StandardTokenType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenClassification {
    Comment,
    Keyword,
    StringLiteral,
    NumberLiteral,
    TypeName,
    FunctionName,
    Variable,
    Operator,
    Punctuation,
    Whitespace,
    Unknown,
}

impl std::fmt::Display for TokenClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenClassification::Comment => write!(f, "comment"),
            TokenClassification::Keyword => write!(f, "keyword"),
            TokenClassification::StringLiteral => write!(f, "string"),
            TokenClassification::NumberLiteral => write!(f, "number"),
            TokenClassification::TypeName => write!(f, "type"),
            TokenClassification::FunctionName => write!(f, "function"),
            TokenClassification::Variable => write!(f, "variable"),
            TokenClassification::Operator => write!(f, "operator"),
            TokenClassification::Punctuation => write!(f, "punctuation"),
            TokenClassification::Whitespace => write!(f, "whitespace"),
            TokenClassification::Unknown => write!(f, "unknown"),
        }
    }
}

/// Classify a token string into a `TokenClassification`.
///
/// This is a heuristic classifier for simple syntax highlighting.
pub fn classify_token(text: &str) -> TokenClassification {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return TokenClassification::Whitespace;
    }

    // Check if it starts with comment markers
    if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('#') {
        return TokenClassification::Comment;
    }

    // Check string literals
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        return TokenClassification::StringLiteral;
    }

    // Check number literals
    if trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
        && trimmed.chars().all(|c| {
            c.is_ascii_hexdigit() || c == '.' || c == 'x' || c == 'b' || c == '_'
        })
    {
        return TokenClassification::NumberLiteral;
    }

    // Check operators
    if trimmed.len() <= 3 && trimmed.chars().all(|c| "+-*/%=<>!&|^~?:.".contains(c)) {
        return TokenClassification::Operator;
    }

    // Check punctuation
    if trimmed.len() == 1 && "(){}[];,".contains(trimmed) {
        return TokenClassification::Punctuation;
    }

    // Check common keywords
    const KEYWORDS: &[&str] = &[
        "fn", "let", "mut", "const", "static", "struct", "enum", "impl", "trait", "pub", "use",
        "mod", "if", "else", "match", "for", "while", "loop", "return", "break", "continue",
        "where", "as", "in", "ref", "self", "super", "crate", "type", "async", "await", "move",
        "dyn", "unsafe",
    ];
    if KEYWORDS.contains(&trimmed) {
        return TokenClassification::Keyword;
    }

    // Check if it looks like a type name (starts with uppercase)
    if trimmed.chars().next().map_or(false, |c| c.is_uppercase()) {
        return TokenClassification::TypeName;
    }

    // Check if it looks like a variable (lowercase alphanumeric with underscores)
    if trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
        && trimmed.chars().next().map_or(false, |c| c.is_lowercase())
    {
        return TokenClassification::Variable;
    }

    TokenClassification::Unknown
}

/// A classified token with its text and classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedToken {
    pub text: String,
    pub classification: TokenClassification,
}

/// Classify all whitespace-separated tokens in a line.
pub fn classify_line_tokens(line: &str) -> Vec<ClassifiedToken> {
    line.split_whitespace()
        .map(|word| ClassifiedToken {
            text: word.to_string(),
            classification: classify_token(word),
        })
        .collect()
}

/// Enhanced statistics counting classifications across tokens.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClassificationStats {
    pub total: usize,
    pub comments: usize,
    pub keywords: usize,
    pub strings: usize,
    pub numbers: usize,
    pub types: usize,
    pub functions: usize,
    pub variables: usize,
    pub operators: usize,
    pub punctuation: usize,
    pub whitespace: usize,
    pub unknown: usize,
}

/// Compute classification statistics for a slice of classified tokens.
pub fn compute_classification_stats(tokens: &[ClassifiedToken]) -> ClassificationStats {
    let mut stats = ClassificationStats {
        total: tokens.len(),
        ..Default::default()
    };
    for t in tokens {
        match t.classification {
            TokenClassification::Comment => stats.comments += 1,
            TokenClassification::Keyword => stats.keywords += 1,
            TokenClassification::StringLiteral => stats.strings += 1,
            TokenClassification::NumberLiteral => stats.numbers += 1,
            TokenClassification::TypeName => stats.types += 1,
            TokenClassification::FunctionName => stats.functions += 1,
            TokenClassification::Variable => stats.variables += 1,
            TokenClassification::Operator => stats.operators += 1,
            TokenClassification::Punctuation => stats.punctuation += 1,
            TokenClassification::Whitespace => stats.whitespace += 1,
            TokenClassification::Unknown => stats.unknown += 1,
        }
    }
    stats
}

/// Map a `TokenClassification` to a semantic color name.
pub fn classification_color_name(cls: TokenClassification) -> &'static str {
    match cls {
        TokenClassification::Comment => "comment.foreground",
        TokenClassification::Keyword => "keyword.foreground",
        TokenClassification::StringLiteral => "string.foreground",
        TokenClassification::NumberLiteral => "number.foreground",
        TokenClassification::TypeName => "type.foreground",
        TokenClassification::FunctionName => "function.foreground",
        TokenClassification::Variable => "variable.foreground",
        TokenClassification::Operator => "operator.foreground",
        TokenClassification::Punctuation => "punctuation.foreground",
        TokenClassification::Whitespace => "editor.foreground",
        TokenClassification::Unknown => "editor.foreground",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_metadata_roundtrip() {
        let meta = TokenMetadata::new(5, StandardTokenType::Comment, FontStyle::BOLD, 100, 50);
        assert_eq!(meta.language_id(), 5);
        assert_eq!(meta.token_type(), StandardTokenType::Comment);
        assert!(meta.font_style().is_bold());
        assert_eq!(meta.foreground(), 100);
        assert_eq!(meta.background(), 50);
    }

    #[test]
    fn font_style_operations() {
        let style = FontStyle::ITALIC | FontStyle::BOLD;
        assert!(style.is_italic());
        assert!(style.is_bold());
        assert!(!style.is_underline());
    }

    #[test]
    fn line_tokens_lookup() {
        let tokens = LineTokens::new(vec![
            Token {
                start_offset: 0,
                metadata: TokenMetadata(0),
            },
            Token {
                start_offset: 5,
                metadata: TokenMetadata(1),
            },
            Token {
                start_offset: 10,
                metadata: TokenMetadata(2),
            },
        ]);

        assert_eq!(tokens.token_at(0).unwrap().metadata.0, 0);
        assert_eq!(tokens.token_at(3).unwrap().metadata.0, 0);
        assert_eq!(tokens.token_at(5).unwrap().metadata.0, 1);
        assert_eq!(tokens.token_at(7).unwrap().metadata.0, 1);
        assert_eq!(tokens.token_at(10).unwrap().metadata.0, 2);
    }

    #[test]
    fn empty_line_tokens() {
        let tokens = LineTokens::empty();
        assert_eq!(tokens.count(), 0);
        assert!(tokens.token_at(0).is_none());
    }

    #[test]
    fn tokenization_state() {
        let s = TokenizationState::initial();
        assert_eq!(s, TokenizationState(0));
    }

    // --- new tests ---

    #[test]
    fn token_metadata_try_new_valid() {
        let meta = TokenMetadata::try_new(1, StandardTokenType::String, FontStyle::ITALIC, 511, 255);
        assert!(meta.is_ok());
        let meta = meta.unwrap();
        assert_eq!(meta.language_id(), 1);
        assert_eq!(meta.token_type(), StandardTokenType::String);
        assert!(meta.font_style().is_italic());
        assert_eq!(meta.foreground(), 511);
        assert_eq!(meta.background(), 255);
    }

    #[test]
    fn token_metadata_try_new_fg_out_of_range() {
        let err = TokenMetadata::try_new(0, StandardTokenType::Other, FontStyle::NONE, 512, 0);
        assert_eq!(err, Err(TokenError::ForegroundOutOfRange(512)));
    }

    #[test]
    fn token_metadata_try_new_bg_out_of_range() {
        let err = TokenMetadata::try_new(0, StandardTokenType::Other, FontStyle::NONE, 0, 256);
        assert_eq!(err, Err(TokenError::BackgroundOutOfRange(256)));
    }

    #[test]
    fn token_metadata_builder() {
        let meta = TokenMetadataBuilder::new()
            .language_id(3)
            .token_type(StandardTokenType::RegExp)
            .font_style(FontStyle::UNDERLINE)
            .foreground(42)
            .background(7)
            .build()
            .unwrap();
        assert_eq!(meta.language_id(), 3);
        assert_eq!(meta.token_type(), StandardTokenType::RegExp);
        assert!(meta.font_style().is_underline());
        assert_eq!(meta.foreground(), 42);
        assert_eq!(meta.background(), 7);
    }

    #[test]
    fn token_metadata_with_helpers() {
        let base = TokenMetadata::new(1, StandardTokenType::Comment, FontStyle::BOLD, 10, 20);
        let changed = base.with_foreground(99);
        assert_eq!(changed.foreground(), 99);
        assert_eq!(changed.background(), 20);
        assert_eq!(changed.language_id(), 1);

        let styled = base.with_font_style(FontStyle::ITALIC);
        assert!(styled.font_style().is_italic());
        assert!(!styled.font_style().is_bold());
    }

    #[test]
    fn font_style_toggle_and_remove() {
        let style = FontStyle::BOLD | FontStyle::ITALIC;
        let toggled = style.toggle(FontStyle::BOLD);
        assert!(!toggled.is_bold());
        assert!(toggled.is_italic());

        let removed = style.remove(FontStyle::ITALIC);
        assert!(removed.is_bold());
        assert!(!removed.is_italic());
    }

    #[test]
    fn font_style_display() {
        assert_eq!(FontStyle::NONE.to_string(), "None");
        assert_eq!((FontStyle::BOLD | FontStyle::UNDERLINE).to_string(), "Bold|Underline");
    }

    #[test]
    fn line_tokens_validate_sorted() {
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: TokenMetadata(0) },
            Token { start_offset: 5, metadata: TokenMetadata(0) },
        ]);
        assert!(lt.validate().is_ok());
    }

    #[test]
    fn line_tokens_validate_unsorted() {
        let lt = LineTokens::new(vec![
            Token { start_offset: 5, metadata: TokenMetadata(0) },
            Token { start_offset: 2, metadata: TokenMetadata(0) },
        ]);
        assert_eq!(lt.validate(), Err(TokenError::OffsetsNotSorted));
    }

    #[test]
    fn line_tokens_token_range() {
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: TokenMetadata(0) },
            Token { start_offset: 5, metadata: TokenMetadata(1) },
            Token { start_offset: 10, metadata: TokenMetadata(2) },
        ]);
        assert_eq!(lt.token_range(0, 20).unwrap(), (0, 5));
        assert_eq!(lt.token_range(1, 20).unwrap(), (5, 10));
        assert_eq!(lt.token_range(2, 20).unwrap(), (10, 20));
        assert!(lt.token_range(3, 20).is_err());
    }

    #[test]
    fn line_tokens_merge() {
        let a = LineTokens::new(vec![
            Token { start_offset: 0, metadata: TokenMetadata(0) },
            Token { start_offset: 10, metadata: TokenMetadata(2) },
        ]);
        let b = LineTokens::new(vec![
            Token { start_offset: 5, metadata: TokenMetadata(1) },
        ]);
        let merged = a.merge(&b);
        assert_eq!(merged.count(), 3);
        assert_eq!(merged.tokens[0].start_offset, 0);
        assert_eq!(merged.tokens[1].start_offset, 5);
        assert_eq!(merged.tokens[2].start_offset, 10);
    }

    #[test]
    fn line_tokens_contains_and_count_type() {
        let comment_meta = TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::NONE, 0, 0);
        let string_meta = TokenMetadata::new(0, StandardTokenType::String, FontStyle::NONE, 0, 0);
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: comment_meta },
            Token { start_offset: 5, metadata: string_meta },
            Token { start_offset: 10, metadata: comment_meta },
        ]);
        assert!(lt.contains_type(StandardTokenType::Comment));
        assert!(!lt.contains_type(StandardTokenType::RegExp));
        assert_eq!(lt.count_type(StandardTokenType::Comment), 2);
        assert_eq!(lt.count_type(StandardTokenType::String), 1);
    }

    #[test]
    fn token_error_display() {
        let err = TokenError::ForegroundOutOfRange(600);
        assert!(err.to_string().contains("600"));
        let err2 = TokenError::IndexOutOfBounds { index: 5, len: 3 };
        assert!(err2.to_string().contains("5"));
    }

    #[test]
    fn tokenization_state_display_and_is_initial() {
        let s = TokenizationState::initial();
        assert!(s.is_initial());
        assert_eq!(s.to_string(), "State(0)");
        let s2 = TokenizationState(42);
        assert!(!s2.is_initial());
    }

    #[test]
    fn token_statistics_computation() {
        let comment_meta = TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::NONE, 0, 0);
        let string_meta = TokenMetadata::new(0, StandardTokenType::String, FontStyle::NONE, 0, 0);
        let other_meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: comment_meta },
            Token { start_offset: 5, metadata: string_meta },
            Token { start_offset: 10, metadata: comment_meta },
            Token { start_offset: 15, metadata: other_meta },
        ]);
        let stats = compute_token_statistics(&lt);
        assert_eq!(stats.total_tokens, 4);
        assert_eq!(stats.comment_count, 2);
        assert_eq!(stats.string_count, 1);
        assert_eq!(stats.other_count, 1);
        assert_eq!(stats.regexp_count, 0);
    }

    #[test]
    fn merge_token_ranges_deduplicates() {
        let meta = TokenMetadata(0);
        let tokens = vec![
            Token { start_offset: 0, metadata: meta },
            Token { start_offset: 0, metadata: TokenMetadata(1) },
            Token { start_offset: 5, metadata: meta },
        ];
        let merged = merge_token_ranges(&tokens);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start_offset, 0);
        assert_eq!(merged[1].start_offset, 5);
    }

    #[test]
    fn token_type_color_mapping() {
        assert_eq!(token_type_color_name(StandardTokenType::Comment), "comment.foreground");
        assert_eq!(token_type_color_name(StandardTokenType::String), "string.foreground");
        assert_eq!(token_type_color_name(StandardTokenType::RegExp), "regexp.foreground");
        assert_eq!(token_type_color_name(StandardTokenType::Other), "editor.foreground");
    }

    #[test]
    fn tokenization_cache_set_and_get() {
        let mut cache = TokenizationCache::new();
        let lt = LineTokens::new(vec![Token { start_offset: 0, metadata: TokenMetadata(0) }]);
        let state = TokenizationState::initial();
        cache.set(0, lt.clone(), state.clone());
        cache.set(2, lt.clone(), state.clone());
        assert!(cache.get(0).is_some());
        assert!(cache.get(1).is_none());
        assert!(cache.get(2).is_some());
        assert_eq!(cache.cached_line_count(), 2);
    }

    #[test]
    fn tokenization_cache_invalidate() {
        let mut cache = TokenizationCache::new();
        let lt = LineTokens::new(vec![Token { start_offset: 0, metadata: TokenMetadata(0) }]);
        let state = TokenizationState::initial();
        cache.set(0, lt.clone(), state.clone());
        cache.set(1, lt.clone(), state.clone());
        cache.set(2, lt.clone(), state.clone());
        assert_eq!(cache.cached_line_count(), 3);
        cache.invalidate(1);
        assert_eq!(cache.cached_line_count(), 1);
        assert!(cache.get(0).is_some());
        assert!(cache.get(1).is_none());
    }

    #[test]
    fn merge_token_ranges_empty() {
        let merged = merge_token_ranges(&[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn token_type_name_all_variants() {
        assert_eq!(token_type_name(StandardTokenType::Other), "Other");
        assert_eq!(token_type_name(StandardTokenType::Comment), "Comment");
        assert_eq!(token_type_name(StandardTokenType::String), "String");
        assert_eq!(token_type_name(StandardTokenType::RegExp), "RegExp");
    }

    #[test]
    fn token_type_name_is_not_empty() {
        for tt in [StandardTokenType::Other, StandardTokenType::Comment, StandardTokenType::String, StandardTokenType::RegExp] {
            assert!(!token_type_name(tt).is_empty());
        }
    }

    #[test]
    fn classify_keyword() {
        assert_eq!(classify_token("fn"), TokenClassification::Keyword);
        assert_eq!(classify_token("let"), TokenClassification::Keyword);
        assert_eq!(classify_token("struct"), TokenClassification::Keyword);
        assert_eq!(classify_token("return"), TokenClassification::Keyword);
    }

    #[test]
    fn classify_comment() {
        assert_eq!(classify_token("// comment"), TokenClassification::Comment);
        assert_eq!(classify_token("/* block */"), TokenClassification::Comment);
    }

    #[test]
    fn classify_string_literal() {
        assert_eq!(classify_token("\"hello\""), TokenClassification::StringLiteral);
        assert_eq!(classify_token("'c'"), TokenClassification::StringLiteral);
    }

    #[test]
    fn classify_number_literal() {
        assert_eq!(classify_token("42"), TokenClassification::NumberLiteral);
        assert_eq!(classify_token("3.14"), TokenClassification::NumberLiteral);
        assert_eq!(classify_token("0xFF"), TokenClassification::NumberLiteral);
    }

    #[test]
    fn classify_operator() {
        assert_eq!(classify_token("+"), TokenClassification::Operator);
        assert_eq!(classify_token("=="), TokenClassification::Operator);
        assert_eq!(classify_token("=>"), TokenClassification::Operator);
    }

    #[test]
    fn classify_punctuation() {
        assert_eq!(classify_token("("), TokenClassification::Punctuation);
        assert_eq!(classify_token(";"), TokenClassification::Punctuation);
        assert_eq!(classify_token("{"), TokenClassification::Punctuation);
    }

    #[test]
    fn classify_type_name() {
        assert_eq!(classify_token("String"), TokenClassification::TypeName);
        assert_eq!(classify_token("Vec"), TokenClassification::TypeName);
    }

    #[test]
    fn classify_variable() {
        assert_eq!(classify_token("my_var"), TokenClassification::Variable);
        assert_eq!(classify_token("count"), TokenClassification::Variable);
    }

    #[test]
    fn classify_whitespace() {
        assert_eq!(classify_token(""), TokenClassification::Whitespace);
        assert_eq!(classify_token("  "), TokenClassification::Whitespace);
    }

    #[test]
    fn classify_line_tokens_works() {
        let tokens = classify_line_tokens("fn main() {");
        assert!(tokens.len() >= 2);
        assert_eq!(tokens[0].classification, TokenClassification::Keyword);
    }

    #[test]
    fn classification_stats_computation() {
        let tokens = vec![
            ClassifiedToken { text: "fn".into(), classification: TokenClassification::Keyword },
            ClassifiedToken { text: "main".into(), classification: TokenClassification::Variable },
            ClassifiedToken { text: "42".into(), classification: TokenClassification::NumberLiteral },
            ClassifiedToken { text: "fn".into(), classification: TokenClassification::Keyword },
        ];
        let stats = compute_classification_stats(&tokens);
        assert_eq!(stats.total, 4);
        assert_eq!(stats.keywords, 2);
        assert_eq!(stats.variables, 1);
        assert_eq!(stats.numbers, 1);
    }

    #[test]
    fn classification_display() {
        assert_eq!(format!("{}", TokenClassification::Comment), "comment");
        assert_eq!(format!("{}", TokenClassification::Keyword), "keyword");
        assert_eq!(format!("{}", TokenClassification::Operator), "operator");
    }

    #[test]
    fn classification_color_names() {
        assert_eq!(classification_color_name(TokenClassification::Comment), "comment.foreground");
        assert_eq!(classification_color_name(TokenClassification::Keyword), "keyword.foreground");
        assert_eq!(classification_color_name(TokenClassification::Unknown), "editor.foreground");
    }
}
