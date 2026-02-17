//! Tokenization infrastructure.
//!
//! Equivalent to VS Code's `vs/editor/common/languages/supports/tokenization.ts`.
//! Provides token types and tokenization results used by syntax highlighting.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// StandardTokenType helpers
// ---------------------------------------------------------------------------

impl StandardTokenType {
    /// Returns a lowercase label for the token type.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Other => "other",
            Self::Comment => "comment",
            Self::String => "string",
            Self::RegExp => "regex",
        }
    }

    /// Returns `true` for textual token types (String and Comment).
    pub fn is_textual(&self) -> bool {
        matches!(self, Self::String | Self::Comment)
    }
}

// ---------------------------------------------------------------------------
// Token helpers
// ---------------------------------------------------------------------------

impl Token {
    /// Returns `true` if this token is a comment.
    pub fn is_comment(&self) -> bool {
        self.metadata.token_type() == StandardTokenType::Comment
    }

    /// Returns `true` if this token is a string.
    pub fn is_string(&self) -> bool {
        self.metadata.token_type() == StandardTokenType::String
    }
}

// ---------------------------------------------------------------------------
// LineTokens – convenience accessors
// ---------------------------------------------------------------------------

impl LineTokens {
    /// Returns the number of comment tokens.
    pub fn comment_count(&self) -> usize {
        self.count_type(StandardTokenType::Comment)
    }

    /// Returns the number of string tokens.
    pub fn string_count(&self) -> usize {
        self.count_type(StandardTokenType::String)
    }

    /// Returns the first token, if any.
    pub fn first(&self) -> Option<&Token> {
        self.tokens.first()
    }

    /// Returns the last token, if any.
    pub fn last(&self) -> Option<&Token> {
        self.tokens.last()
    }
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

// ---------------------------------------------------------------------------
// TokenScope
// ---------------------------------------------------------------------------

/// Scope-based token classification with dot-delimited scope names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenScope {
    /// Dot-delimited scope segments, e.g. "comment.line.double-slash".
    pub scope: String,
}

impl TokenScope {
    /// Create a new token scope.
    pub fn new(scope: impl Into<String>) -> Self {
        Self {
            scope: scope.into(),
        }
    }

    /// Return the number of scope segments.
    pub fn depth(&self) -> usize {
        if self.scope.is_empty() {
            return 0;
        }
        self.scope.split('.').count()
    }

    /// Return the top-level scope segment (e.g. "comment" from "comment.line.double-slash").
    pub fn root(&self) -> &str {
        self.scope.split('.').next().unwrap_or("")
    }

    /// Return the leaf scope segment.
    pub fn leaf(&self) -> &str {
        self.scope.rsplit('.').next().unwrap_or("")
    }

    /// Check if this scope is a prefix of another.
    pub fn is_prefix_of(&self, other: &TokenScope) -> bool {
        other.scope.starts_with(&self.scope)
            && (other.scope.len() == self.scope.len()
                || other.scope.as_bytes().get(self.scope.len()) == Some(&b'.'))
    }

    /// Return the parent scope (one level up), or `None` if already root.
    pub fn parent(&self) -> Option<TokenScope> {
        self.scope
            .rfind('.')
            .map(|pos| TokenScope::new(&self.scope[..pos]))
    }
}

impl fmt::Display for TokenScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.scope)
    }
}

// ---------------------------------------------------------------------------
// TokenDiffEngine
// ---------------------------------------------------------------------------

/// Describes a change between two token sequences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenDiffOp {
    /// Token at old index was kept (identical in both).
    Keep { index: usize },
    /// Token was inserted at new index.
    Insert { new_index: usize },
    /// Token was removed from old index.
    Remove { old_index: usize },
}

/// Diffs two token sequences.
pub struct TokenDiffEngine;

impl TokenDiffEngine {
    /// Compute a simple diff between old and new token sequences.
    pub fn diff(old: &[Token], new: &[Token]) -> Vec<TokenDiffOp> {
        let mut ops = Vec::new();
        let mut oi = 0;
        let mut ni = 0;
        while oi < old.len() && ni < new.len() {
            if old[oi] == new[ni] {
                ops.push(TokenDiffOp::Keep { index: oi });
                oi += 1;
                ni += 1;
            } else if oi + 1 < old.len() && old[oi + 1] == new[ni] {
                ops.push(TokenDiffOp::Remove { old_index: oi });
                oi += 1;
            } else {
                ops.push(TokenDiffOp::Insert { new_index: ni });
                ni += 1;
            }
        }
        while oi < old.len() {
            ops.push(TokenDiffOp::Remove { old_index: oi });
            oi += 1;
        }
        while ni < new.len() {
            ops.push(TokenDiffOp::Insert { new_index: ni });
            ni += 1;
        }
        ops
    }

    /// Returns `true` if two token sequences are identical.
    pub fn is_identical(old: &[Token], new: &[Token]) -> bool {
        old == new
    }

    /// Count insertions in a diff.
    pub fn insertion_count(ops: &[TokenDiffOp]) -> usize {
        ops.iter()
            .filter(|op| matches!(op, TokenDiffOp::Insert { .. }))
            .count()
    }

    /// Count removals in a diff.
    pub fn removal_count(ops: &[TokenDiffOp]) -> usize {
        ops.iter()
            .filter(|op| matches!(op, TokenDiffOp::Remove { .. }))
            .count()
    }
}

// ---------------------------------------------------------------------------
// TokenStreamIterator
// ---------------------------------------------------------------------------

/// Iterates over tokens providing context (previous and next token).
pub struct TokenStreamIterator<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> TokenStreamIterator<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Peek at the previous token without advancing.
    pub fn prev(&self) -> Option<&'a Token> {
        if self.pos > 0 {
            Some(&self.tokens[self.pos - 1])
        } else {
            None
        }
    }

    /// Peek at the current token without advancing.
    pub fn current(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    /// Peek at the next token without advancing.
    pub fn peek_next(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos + 1)
    }

    /// Advance and return the current token.
    pub fn advance(&mut self) -> Option<&'a Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    /// Return the remaining number of tokens.
    pub fn remaining(&self) -> usize {
        self.tokens.len().saturating_sub(self.pos)
    }

    /// Reset position to the beginning.
    pub fn reset(&mut self) {
        self.pos = 0;
    }
}

// ---------------------------------------------------------------------------
// TokenizationCache – eviction & stats
// ---------------------------------------------------------------------------

impl TokenizationCache {
    /// Evict the oldest `n` cached entries (from the start).
    pub fn evict_oldest(&mut self, n: usize) {
        let mut evicted = 0;
        for entry in self.entries.iter_mut() {
            if evicted >= n {
                break;
            }
            if entry.is_some() {
                *entry = None;
                evicted += 1;
            }
        }
    }

    /// Return the hit rate as the ratio of cached lines to total capacity.
    pub fn hit_rate(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }
        self.cached_line_count() as f64 / self.entries.len() as f64
    }

    /// Trim the cache to keep only the first `max_lines` entries.
    pub fn trim_to(&mut self, max_lines: usize) {
        if self.entries.len() > max_lines {
            self.entries.truncate(max_lines);
        }
    }
}

// ---------------------------------------------------------------------------
// TokenRangeExtractor – extract text ranges from tokens
// ---------------------------------------------------------------------------

/// Extract a specific token's text from the source line given the token index
/// and the full line text.
pub fn extract_token_text(tokens: &LineTokens, index: usize, line: &str) -> Option<String> {
    if index >= tokens.tokens.len() {
        return None;
    }
    let start = tokens.tokens[index].start_offset as usize;
    let end = if index + 1 < tokens.tokens.len() {
        tokens.tokens[index + 1].start_offset as usize
    } else {
        line.len()
    };
    if start > line.len() || end > line.len() || start > end {
        return None;
    }
    Some(line[start..end].to_string())
}

/// Count how many tokens of each `StandardTokenType` appear in a `LineTokens`.
pub fn token_type_histogram(tokens: &LineTokens) -> std::collections::HashMap<StandardTokenType, usize> {
    let mut map = std::collections::HashMap::new();
    for t in &tokens.tokens {
        *map.entry(t.metadata.token_type()).or_insert(0) += 1;
    }
    map
}

// ---------------------------------------------------------------------------
// TokenSummary – high-level summary of tokenization results
// ---------------------------------------------------------------------------

/// A high-level summary of tokens on a line.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenSummary {
    pub total_tokens: usize,
    pub comment_count: usize,
    pub string_count: usize,
    pub regexp_count: usize,
    pub other_count: usize,
    pub has_bold: bool,
    pub has_italic: bool,
}

impl TokenSummary {
    /// Create a summary from a `LineTokens` instance.
    pub fn from_line_tokens(lt: &LineTokens) -> Self {
        let mut comment_count = 0;
        let mut string_count = 0;
        let mut regexp_count = 0;
        let mut other_count = 0;
        let mut has_bold = false;
        let mut has_italic = false;
        for t in &lt.tokens {
            match t.metadata.token_type() {
                StandardTokenType::Comment => comment_count += 1,
                StandardTokenType::String => string_count += 1,
                StandardTokenType::RegExp => regexp_count += 1,
                StandardTokenType::Other => other_count += 1,
            }
            let style = t.metadata.font_style();
            if style.is_bold() {
                has_bold = true;
            }
            if style.is_italic() {
                has_italic = true;
            }
        }
        Self {
            total_tokens: lt.tokens.len(),
            comment_count,
            string_count,
            regexp_count,
            other_count,
            has_bold,
            has_italic,
        }
    }

    /// Return the dominant token type (the one with highest count).
    pub fn dominant_type(&self) -> StandardTokenType {
        let counts = [
            (StandardTokenType::Other, self.other_count),
            (StandardTokenType::Comment, self.comment_count),
            (StandardTokenType::String, self.string_count),
            (StandardTokenType::RegExp, self.regexp_count),
        ];
        counts.into_iter().max_by_key(|(_, c)| *c).map(|(t, _)| t).unwrap_or(StandardTokenType::Other)
    }
}

impl fmt::Display for TokenSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Tokens: {} (comment={}, string={}, regexp={}, other={})",
            self.total_tokens, self.comment_count, self.string_count,
            self.regexp_count, self.other_count,
        )
    }
}

/// Validate that all tokens in a `LineTokens` have strictly increasing start offsets.
pub fn validate_token_offsets(lt: &LineTokens) -> bool {
    lt.tokens.windows(2).all(|w| w[0].start_offset < w[1].start_offset)
}

/// Find the token index at a given character offset within a line.
pub fn token_at_offset(lt: &LineTokens, offset: u32) -> Option<usize> {
    if lt.tokens.is_empty() {
        return None;
    }
    let mut result = 0;
    for (i, t) in lt.tokens.iter().enumerate() {
        if t.start_offset <= offset {
            result = i;
        } else {
            break;
        }
    }
    Some(result)
}

// ---------------------------------------------------------------------------
// TokenThemeMapper – scope-to-color mapping
// ---------------------------------------------------------------------------

/// Maps scope names to hex color strings for theme-based token coloring.
#[derive(Debug, Clone, Default)]
pub struct TokenThemeMapper {
    scope_colors: std::collections::HashMap<String, String>,
}

impl TokenThemeMapper {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_scope(&mut self, scope: &str, color: &str) {
        self.scope_colors
            .insert(scope.to_string(), color.to_string());
    }

    pub fn color_for_scope(&self, scope: &str) -> Option<&str> {
        self.scope_colors.get(scope).map(|s| s.as_str())
    }

    /// Look up a color using the token type's name as the scope key.
    pub fn color_for_token_type(&self, tt: StandardTokenType) -> Option<&str> {
        self.color_for_scope(token_type_name(tt))
    }

    pub fn remove_scope(&mut self, scope: &str) -> bool {
        self.scope_colors.remove(scope).is_some()
    }

    pub fn scope_count(&self) -> usize {
        self.scope_colors.len()
    }

    /// Return all scope/color pairs sorted by scope name.
    pub fn all_scopes(&self) -> Vec<(&str, &str)> {
        let mut pairs: Vec<(&str, &str)> = self
            .scope_colors
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        pairs.sort_by_key(|(scope, _)| *scope);
        pairs
    }
}

// ---------------------------------------------------------------------------
// SemanticTokenDelta – incremental token edits
// ---------------------------------------------------------------------------

/// A single edit operation on a flat `Vec<u32>` semantic-token stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTokenEdit {
    pub start: u32,
    pub delete_count: u32,
    pub data: Vec<u32>,
}

/// Accumulates edits and applies them to a semantic-token data array.
#[derive(Debug, Clone, Default)]
pub struct SemanticTokenDelta {
    pub edits: Vec<SemanticTokenEdit>,
}

impl SemanticTokenDelta {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_edit(&mut self, start: u32, delete_count: u32, data: Vec<u32>) {
        self.edits.push(SemanticTokenEdit {
            start,
            delete_count,
            data,
        });
    }

    /// Apply all edits in reverse order so earlier indices stay valid.
    pub fn apply(&self, tokens: &mut Vec<u32>) {
        let mut sorted: Vec<&SemanticTokenEdit> = self.edits.iter().collect();
        sorted.sort_by(|a, b| b.start.cmp(&a.start));
        for edit in sorted {
            let start = edit.start as usize;
            let end = start + edit.delete_count as usize;
            let end = end.min(tokens.len());
            tokens.splice(start..end, edit.data.iter().copied());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }

    pub fn total_insertions(&self) -> u32 {
        self.edits.iter().map(|e| e.data.len() as u32).sum()
    }

    pub fn total_deletions(&self) -> u32 {
        self.edits.iter().map(|e| e.delete_count).sum()
    }
}

// ---------------------------------------------------------------------------
// TokenInspector – detailed per-token inspection
// ---------------------------------------------------------------------------

/// Detailed information about a single token extracted from its line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenDetail {
    pub text: String,
    pub token_type: StandardTokenType,
    pub start: u32,
    pub end: u32,
    pub length: u32,
}

/// Inspects tokens against the source line to produce `TokenDetail`s.
#[derive(Debug, Clone, Default)]
pub struct TokenInspector;

impl TokenInspector {
    /// Inspect a single token within `line`, computing its end from `line_len`.
    pub fn inspect_token(&self, token: &Token, next_offset: u32, line: &str) -> TokenDetail {
        let start = token.start_offset;
        let end = next_offset;
        let s = start as usize;
        let e = (end as usize).min(line.len());
        let text = if s <= e && s <= line.len() {
            line[s..e].to_string()
        } else {
            String::new()
        };
        TokenDetail {
            text,
            token_type: token.metadata.token_type(),
            start,
            end,
            length: end.saturating_sub(start),
        }
    }

    /// Inspect every token in a `LineTokens` against the source line.
    pub fn inspect_line(&self, tokens: &LineTokens, line: &str) -> Vec<TokenDetail> {
        let line_len = line.len() as u32;
        tokens
            .tokens
            .iter()
            .enumerate()
            .map(|(i, tok)| {
                let next = tokens
                    .tokens
                    .get(i + 1)
                    .map(|t| t.start_offset)
                    .unwrap_or(line_len);
                self.inspect_token(tok, next, line)
            })
            .collect()
    }

    /// Produce a one-line summary of a set of token details.
    pub fn summary(details: &[TokenDetail]) -> String {
        if details.is_empty() {
            return "no tokens".to_string();
        }
        let total = details.len();
        let total_len: u32 = details.iter().map(|d| d.length).sum();
        format!("{total} token(s), {total_len} char(s)")
    }
}

// ---------------------------------------------------------------------------
// TokenPerformanceProfiler – simple timing profiler
// ---------------------------------------------------------------------------

/// Collects labelled duration measurements for tokenization performance analysis.
#[derive(Debug, Clone, Default)]
pub struct TokenPerformanceProfiler {
    timings: Vec<(String, u64)>,
}

impl TokenPerformanceProfiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, label: &str, duration_us: u64) {
        self.timings.push((label.to_string(), duration_us));
    }

    pub fn average(&self) -> f64 {
        if self.timings.is_empty() {
            return 0.0;
        }
        self.total() as f64 / self.timings.len() as f64
    }

    pub fn max_timing(&self) -> Option<(&str, u64)> {
        self.timings
            .iter()
            .max_by_key(|(_, d)| *d)
            .map(|(l, d)| (l.as_str(), *d))
    }

    pub fn min_timing(&self) -> Option<(&str, u64)> {
        self.timings
            .iter()
            .min_by_key(|(_, d)| *d)
            .map(|(l, d)| (l.as_str(), *d))
    }

    pub fn total(&self) -> u64 {
        self.timings.iter().map(|(_, d)| *d).sum()
    }

    pub fn count(&self) -> usize {
        self.timings.len()
    }

    pub fn clear(&mut self) {
        self.timings.clear();
    }

    /// Produce a multi-line human-readable report.
    pub fn report(&self) -> String {
        if self.timings.is_empty() {
            return "no timings recorded".to_string();
        }
        let mut lines = Vec::with_capacity(self.timings.len() + 3);
        lines.push(format!("Profiler report ({} entries):", self.count()));
        for (label, us) in &self.timings {
            lines.push(format!("  {label}: {us}µs"));
        }
        lines.push(format!("  total: {}µs, avg: {:.1}µs", self.total(), self.average()));
        lines.join("\n")
    }
}


// === Token Performance Monitor ===

/// Token Performance Monitor implementation.
#[derive(Debug, Clone)]
pub struct TokenPerformanceMonitor {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TokenPerformanceMonitorStats,
}

/// Statistics for TokenPerformanceMonitor.
#[derive(Debug, Clone, Default)]
pub struct TokenPerformanceMonitorStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TokenPerformanceMonitorStats {
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

impl TokenPerformanceMonitor {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TokenPerformanceMonitorStats::default(),
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

    pub fn stats(&self) -> &TokenPerformanceMonitorStats {
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

impl Default for TokenPerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

// === Token Encoding Optimizer ===

/// Priority level for TokenEncodingOptimizer items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TokenEncodingOptimizerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TokenEncodingOptimizerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TokenEncodingOptimizerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Token Encoding Optimizer implementation.
#[derive(Debug, Clone)]
pub struct TokenEncodingOptimizer {
    items: Vec<TokenEncodingOptimizerItem>,
    max_items: usize,
    default_priority: TokenEncodingOptimizerPriority,
}

/// A single item in TokenEncodingOptimizer.
#[derive(Debug, Clone)]
pub struct TokenEncodingOptimizerItem {
    pub id: String,
    pub label: String,
    pub priority: TokenEncodingOptimizerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TokenEncodingOptimizerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TokenEncodingOptimizerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TokenEncodingOptimizerPriority) -> Self {
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

impl TokenEncodingOptimizer {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TokenEncodingOptimizerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TokenEncodingOptimizerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TokenEncodingOptimizerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TokenEncodingOptimizerItem> {
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

    pub fn by_priority(&self, priority: TokenEncodingOptimizerPriority) -> Vec<&TokenEncodingOptimizerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TokenEncodingOptimizerItem> {
        let mut sorted: Vec<&TokenEncodingOptimizerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TokenEncodingOptimizerItem> {
        let mut sorted: Vec<&TokenEncodingOptimizerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TokenEncodingOptimizerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TokenEncodingOptimizerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TokenEncodingOptimizerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TokenEncodingOptimizerItem> {
        self.items.iter()
    }
}

impl Default for TokenEncodingOptimizer {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-tokens: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokensXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl TokensXConfig {
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

impl std::fmt::Display for TokensXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct TokensXRegistry {
    entries: Vec<TokensXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl TokensXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: TokensXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&TokensXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut TokensXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<TokensXConfig> {
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

    pub fn active_entries(&self) -> Vec<&TokensXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&TokensXConfig> {
        let mut sorted: Vec<&TokensXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&TokensXConfig> {
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

    pub fn iter(&self) -> TokensXIterator<'_> {
        TokensXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct TokensXIterator<'a> {
    inner: std::slice::Iter<'a, TokensXConfig>,
}

impl<'a> Iterator for TokensXIterator<'a> {
    type Item = &'a TokensXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct TokensXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl TokensXCache {
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
pub struct TokensXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl TokensXFormatter {
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

    pub fn format_entry(&self, entry: &TokensXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &TokensXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &TokensXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for TokensXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct TokensXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl TokensXValidator {
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

    pub fn validate(&self, entry: &TokensXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &TokensXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for TokensXValidator {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn standard_token_type_label() {
        assert_eq!(StandardTokenType::Other.label(), "other");
        assert_eq!(StandardTokenType::Comment.label(), "comment");
        assert_eq!(StandardTokenType::String.label(), "string");
        assert_eq!(StandardTokenType::RegExp.label(), "regex");
    }

    #[test]
    fn standard_token_type_is_textual() {
        assert!(StandardTokenType::Comment.is_textual());
        assert!(StandardTokenType::String.is_textual());
        assert!(!StandardTokenType::Other.is_textual());
        assert!(!StandardTokenType::RegExp.is_textual());
    }

    #[test]
    fn font_style_is_none() {
        assert!(FontStyle::NONE.is_none());
        assert!(!FontStyle::BOLD.is_none());
        assert!(!(FontStyle::ITALIC | FontStyle::BOLD).is_none());
    }

    #[test]
    fn token_is_comment_and_is_string() {
        let comment_meta = TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::NONE, 0, 0);
        let string_meta = TokenMetadata::new(0, StandardTokenType::String, FontStyle::NONE, 0, 0);
        let other_meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);

        let comment_tok = Token { start_offset: 0, metadata: comment_meta };
        let string_tok = Token { start_offset: 5, metadata: string_meta };
        let other_tok = Token { start_offset: 10, metadata: other_meta };

        assert!(comment_tok.is_comment());
        assert!(!comment_tok.is_string());
        assert!(string_tok.is_string());
        assert!(!string_tok.is_comment());
        assert!(!other_tok.is_comment());
        assert!(!other_tok.is_string());
    }

    #[test]
    fn line_tokens_comment_and_string_count() {
        let comment_meta = TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::NONE, 0, 0);
        let string_meta = TokenMetadata::new(0, StandardTokenType::String, FontStyle::NONE, 0, 0);
        let other_meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: comment_meta },
            Token { start_offset: 5, metadata: string_meta },
            Token { start_offset: 10, metadata: comment_meta },
            Token { start_offset: 15, metadata: other_meta },
            Token { start_offset: 20, metadata: string_meta },
        ]);
        assert_eq!(lt.comment_count(), 2);
        assert_eq!(lt.string_count(), 2);
    }

    #[test]
    fn line_tokens_first_and_last() {
        let meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: meta },
            Token { start_offset: 10, metadata: meta },
            Token { start_offset: 20, metadata: meta },
        ]);
        assert_eq!(lt.first().unwrap().start_offset, 0);
        assert_eq!(lt.last().unwrap().start_offset, 20);

        let empty = LineTokens::empty();
        assert!(empty.first().is_none());
        assert!(empty.last().is_none());
    }

    #[test]
    fn line_tokens_empty_comment_string_count() {
        let empty = LineTokens::empty();
        assert_eq!(empty.comment_count(), 0);
        assert_eq!(empty.string_count(), 0);
    }

    // -- TokenScope --

    #[test]
    fn token_scope_depth_and_root() {
        let scope = TokenScope::new("comment.line.double-slash");
        assert_eq!(scope.depth(), 3);
        assert_eq!(scope.root(), "comment");
        assert_eq!(scope.leaf(), "double-slash");
    }

    #[test]
    fn token_scope_parent() {
        let scope = TokenScope::new("comment.line.double-slash");
        let parent = scope.parent().unwrap();
        assert_eq!(parent.scope, "comment.line");
        let grandparent = parent.parent().unwrap();
        assert_eq!(grandparent.scope, "comment");
        assert!(grandparent.parent().is_none());
    }

    #[test]
    fn token_scope_is_prefix_of() {
        let comment = TokenScope::new("comment");
        let comment_line = TokenScope::new("comment.line");
        assert!(comment.is_prefix_of(&comment_line));
        assert!(!comment_line.is_prefix_of(&comment));
        assert!(comment.is_prefix_of(&comment)); // prefix of itself
    }

    #[test]
    fn token_scope_empty() {
        let empty = TokenScope::new("");
        assert_eq!(empty.depth(), 0);
        assert_eq!(empty.root(), "");
    }

    // -- TokenDiffEngine --

    #[test]
    fn diff_identical_sequences() {
        let meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let tokens = vec![
            Token { start_offset: 0, metadata: meta },
            Token { start_offset: 5, metadata: meta },
        ];
        let ops = TokenDiffEngine::diff(&tokens, &tokens);
        assert_eq!(TokenDiffEngine::insertion_count(&ops), 0);
        assert_eq!(TokenDiffEngine::removal_count(&ops), 0);
        assert!(TokenDiffEngine::is_identical(&tokens, &tokens));
    }

    #[test]
    fn diff_detects_insertion() {
        let m1 = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let m2 = TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::NONE, 0, 0);
        let old = vec![Token { start_offset: 0, metadata: m1 }];
        let new = vec![
            Token { start_offset: 0, metadata: m1 },
            Token { start_offset: 5, metadata: m2 },
        ];
        let ops = TokenDiffEngine::diff(&old, &new);
        assert_eq!(TokenDiffEngine::insertion_count(&ops), 1);
    }

    #[test]
    fn diff_detects_removal() {
        let m1 = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let m2 = TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::NONE, 0, 0);
        let old = vec![
            Token { start_offset: 0, metadata: m1 },
            Token { start_offset: 5, metadata: m2 },
        ];
        let new = vec![Token { start_offset: 0, metadata: m1 }];
        let ops = TokenDiffEngine::diff(&old, &new);
        assert_eq!(TokenDiffEngine::removal_count(&ops), 1);
    }

    // -- TokenStreamIterator --

    #[test]
    fn stream_iterator_navigation() {
        let meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let tokens = vec![
            Token { start_offset: 0, metadata: meta },
            Token { start_offset: 5, metadata: meta },
            Token { start_offset: 10, metadata: meta },
        ];
        let mut iter = TokenStreamIterator::new(&tokens);
        assert!(iter.prev().is_none());
        assert_eq!(iter.current().unwrap().start_offset, 0);
        assert_eq!(iter.peek_next().unwrap().start_offset, 5);
        assert_eq!(iter.remaining(), 3);

        iter.advance();
        assert_eq!(iter.prev().unwrap().start_offset, 0);
        assert_eq!(iter.current().unwrap().start_offset, 5);
        assert_eq!(iter.remaining(), 2);

        iter.advance();
        iter.advance();
        assert!(iter.current().is_none());
        assert_eq!(iter.remaining(), 0);

        iter.reset();
        assert_eq!(iter.remaining(), 3);
    }

    // -- TokenizationCache eviction --

    #[test]
    fn cache_evict_oldest() {
        let meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let mut cache = TokenizationCache::new();
        cache.set(0, LineTokens::new(vec![Token { start_offset: 0, metadata: meta }]), TokenizationState::initial());
        cache.set(1, LineTokens::new(vec![Token { start_offset: 0, metadata: meta }]), TokenizationState::initial());
        cache.set(2, LineTokens::new(vec![Token { start_offset: 0, metadata: meta }]), TokenizationState::initial());
        assert_eq!(cache.cached_line_count(), 3);
        cache.evict_oldest(2);
        assert_eq!(cache.cached_line_count(), 1);
    }

    #[test]
    fn cache_hit_rate() {
        let meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let mut cache = TokenizationCache::new();
        cache.set(0, LineTokens::new(vec![Token { start_offset: 0, metadata: meta }]), TokenizationState::initial());
        cache.set(2, LineTokens::new(vec![Token { start_offset: 0, metadata: meta }]), TokenizationState::initial());
        // capacity is 3 (indices 0,1,2), 2 filled
        let rate = cache.hit_rate();
        assert!((rate - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn cache_trim_to() {
        let meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let mut cache = TokenizationCache::new();
        for i in 0..5 {
            cache.set(i, LineTokens::new(vec![Token { start_offset: 0, metadata: meta }]), TokenizationState::initial());
        }
        assert_eq!(cache.capacity(), 5);
        cache.trim_to(3);
        assert_eq!(cache.capacity(), 3);
    }

    #[test]
    fn test_extract_token_text() {
        let meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: meta },
            Token { start_offset: 3, metadata: meta },
            Token { start_offset: 5, metadata: meta },
        ]);
        assert_eq!(extract_token_text(&lt, 0, "fn foo()"), Some("fn ".to_string()));
        assert_eq!(extract_token_text(&lt, 1, "fn foo()"), Some("fo".to_string()));
        assert_eq!(extract_token_text(&lt, 2, "fn foo()"), Some("o()".to_string()));
        assert_eq!(extract_token_text(&lt, 5, "fn foo()"), None);
    }

    #[test]
    fn test_token_type_histogram() {
        let m_other = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let m_comment = TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::NONE, 0, 0);
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: m_other },
            Token { start_offset: 5, metadata: m_comment },
            Token { start_offset: 10, metadata: m_comment },
        ]);
        let hist = token_type_histogram(&lt);
        assert_eq!(hist.get(&StandardTokenType::Other), Some(&1));
        assert_eq!(hist.get(&StandardTokenType::Comment), Some(&2));
    }

    #[test]
    fn test_token_summary() {
        let m_comment = TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::BOLD, 0, 0);
        let m_string = TokenMetadata::new(0, StandardTokenType::String, FontStyle::ITALIC, 0, 0);
        let m_other = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: m_comment },
            Token { start_offset: 5, metadata: m_string },
            Token { start_offset: 10, metadata: m_other },
        ]);
        let summary = TokenSummary::from_line_tokens(&lt);
        assert_eq!(summary.total_tokens, 3);
        assert_eq!(summary.comment_count, 1);
        assert_eq!(summary.string_count, 1);
        assert_eq!(summary.other_count, 1);
        assert!(summary.has_bold);
        assert!(summary.has_italic);
    }

    #[test]
    fn test_token_summary_dominant_type() {
        let m = TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::NONE, 0, 0);
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: m },
            Token { start_offset: 5, metadata: m },
        ]);
        let summary = TokenSummary::from_line_tokens(&lt);
        assert_eq!(summary.dominant_type(), StandardTokenType::Comment);
    }

    #[test]
    fn test_validate_token_offsets() {
        let meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let valid = LineTokens::new(vec![
            Token { start_offset: 0, metadata: meta },
            Token { start_offset: 5, metadata: meta },
            Token { start_offset: 10, metadata: meta },
        ]);
        assert!(validate_token_offsets(&valid));

        let invalid = LineTokens::new(vec![
            Token { start_offset: 0, metadata: meta },
            Token { start_offset: 0, metadata: meta },
        ]);
        assert!(!validate_token_offsets(&invalid));
    }

    #[test]
    fn test_token_at_offset() {
        let meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: meta },
            Token { start_offset: 5, metadata: meta },
            Token { start_offset: 10, metadata: meta },
        ]);
        assert_eq!(token_at_offset(&lt, 0), Some(0));
        assert_eq!(token_at_offset(&lt, 3), Some(0));
        assert_eq!(token_at_offset(&lt, 5), Some(1));
        assert_eq!(token_at_offset(&lt, 7), Some(1));
        assert_eq!(token_at_offset(&lt, 10), Some(2));
        assert_eq!(token_at_offset(&lt, 15), Some(2));
    }

    // -----------------------------------------------------------------------
    // TokenThemeMapper tests
    // -----------------------------------------------------------------------

    #[test]
    fn theme_mapper_add_and_lookup() {
        let mut mapper = TokenThemeMapper::new();
        mapper.add_scope("Comment", "#00ff00");
        mapper.add_scope("String", "#ff0000");
        assert_eq!(mapper.color_for_scope("Comment"), Some("#00ff00"));
        assert_eq!(mapper.color_for_scope("String"), Some("#ff0000"));
        assert_eq!(mapper.color_for_scope("Other"), None);
        assert_eq!(mapper.scope_count(), 2);
    }

    #[test]
    fn theme_mapper_color_for_token_type() {
        let mut mapper = TokenThemeMapper::new();
        mapper.add_scope("Comment", "#aabbcc");
        assert_eq!(
            mapper.color_for_token_type(StandardTokenType::Comment),
            Some("#aabbcc")
        );
        assert_eq!(mapper.color_for_token_type(StandardTokenType::Other), None);
    }

    #[test]
    fn theme_mapper_remove_and_all_scopes() {
        let mut mapper = TokenThemeMapper::new();
        mapper.add_scope("B", "#222");
        mapper.add_scope("A", "#111");
        mapper.add_scope("C", "#333");
        let scopes = mapper.all_scopes();
        assert_eq!(scopes, vec![("A", "#111"), ("B", "#222"), ("C", "#333")]);
        assert!(mapper.remove_scope("B"));
        assert!(!mapper.remove_scope("B"));
        assert_eq!(mapper.scope_count(), 2);
    }

    // -----------------------------------------------------------------------
    // SemanticTokenDelta tests
    // -----------------------------------------------------------------------

    #[test]
    fn semantic_delta_apply_insert() {
        let mut delta = SemanticTokenDelta::new();
        delta.add_edit(2, 0, vec![99, 100]);
        let mut tokens = vec![1, 2, 3, 4];
        delta.apply(&mut tokens);
        assert_eq!(tokens, vec![1, 2, 99, 100, 3, 4]);
    }

    #[test]
    fn semantic_delta_apply_delete() {
        let mut delta = SemanticTokenDelta::new();
        delta.add_edit(1, 2, vec![]);
        let mut tokens = vec![10, 20, 30, 40];
        delta.apply(&mut tokens);
        assert_eq!(tokens, vec![10, 40]);
    }

    #[test]
    fn semantic_delta_apply_replace_reverse_order() {
        let mut delta = SemanticTokenDelta::new();
        delta.add_edit(0, 1, vec![55]);
        delta.add_edit(3, 1, vec![66]);
        let mut tokens = vec![1, 2, 3, 4, 5];
        delta.apply(&mut tokens);
        assert_eq!(tokens, vec![55, 2, 3, 66, 5]);
    }

    #[test]
    fn semantic_delta_counts() {
        let mut delta = SemanticTokenDelta::new();
        assert!(delta.is_empty());
        delta.add_edit(0, 3, vec![1, 2]);
        delta.add_edit(5, 1, vec![9, 8, 7]);
        assert_eq!(delta.edit_count(), 2);
        assert_eq!(delta.total_deletions(), 4);
        assert_eq!(delta.total_insertions(), 5);
    }

    // -----------------------------------------------------------------------
    // TokenInspector tests
    // -----------------------------------------------------------------------

    #[test]
    fn inspector_inspect_line() {
        let meta_c = TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::NONE, 0, 0);
        let meta_o = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 0, 0);
        let lt = LineTokens::new(vec![
            Token { start_offset: 0, metadata: meta_o },
            Token { start_offset: 5, metadata: meta_c },
        ]);
        let inspector = TokenInspector;
        let details = inspector.inspect_line(&lt, "hello// hi");
        assert_eq!(details.len(), 2);
        assert_eq!(details[0].text, "hello");
        assert_eq!(details[0].token_type, StandardTokenType::Other);
        assert_eq!(details[0].length, 5);
        assert_eq!(details[1].text, "// hi");
        assert_eq!(details[1].token_type, StandardTokenType::Comment);
    }

    #[test]
    fn inspector_summary() {
        let details = vec![
            TokenDetail { text: "fn".into(), token_type: StandardTokenType::Other, start: 0, end: 2, length: 2 },
            TokenDetail { text: "main".into(), token_type: StandardTokenType::Other, start: 3, end: 7, length: 4 },
        ];
        let s = TokenInspector::summary(&details);
        assert_eq!(s, "2 token(s), 6 char(s)");
        assert_eq!(TokenInspector::summary(&[]), "no tokens");
    }

    // -----------------------------------------------------------------------
    // TokenPerformanceProfiler tests
    // -----------------------------------------------------------------------

    #[test]
    fn profiler_basic_stats() {
        let mut profiler = TokenPerformanceProfiler::new();
        profiler.record("tokenize_line_1", 100);
        profiler.record("tokenize_line_2", 200);
        profiler.record("tokenize_line_3", 300);
        assert_eq!(profiler.count(), 3);
        assert_eq!(profiler.total(), 600);
        assert!((profiler.average() - 200.0).abs() < f64::EPSILON);
        assert_eq!(profiler.max_timing(), Some(("tokenize_line_3", 300)));
        assert_eq!(profiler.min_timing(), Some(("tokenize_line_1", 100)));
    }

    #[test]
    fn profiler_clear_and_report() {
        let mut profiler = TokenPerformanceProfiler::new();
        assert_eq!(profiler.report(), "no timings recorded");
        profiler.record("a", 50);
        profiler.record("b", 150);
        let report = profiler.report();
        assert!(report.contains("a: 50µs"));
        assert!(report.contains("total: 200µs"));
        profiler.clear();
        assert_eq!(profiler.count(), 0);
        assert_eq!(profiler.total(), 0);
    }

    #[test]
    fn tokenPerformanceMonitor_new() {
        let s = TokenPerformanceMonitor::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn tokenPerformanceMonitor_add_contains() {
        let mut s = TokenPerformanceMonitor::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn tokenPerformanceMonitor_add_duplicate() {
        let mut s = TokenPerformanceMonitor::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn tokenPerformanceMonitor_remove() {
        let mut s = TokenPerformanceMonitor::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn tokenPerformanceMonitor_capacity() {
        let s = TokenPerformanceMonitor::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn tokenPerformanceMonitor_search() {
        let mut s = TokenPerformanceMonitor::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn tokenPerformanceMonitor_stats() {
        let mut s = TokenPerformanceMonitor::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn tokenEncodingOptimizer_new() {
        let m = TokenEncodingOptimizer::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn tokenEncodingOptimizer_add_find() {
        let mut m = TokenEncodingOptimizer::new();
        m.add(TokenEncodingOptimizerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn tokenEncodingOptimizer_priority_filter() {
        let mut m = TokenEncodingOptimizer::new();
        m.add(TokenEncodingOptimizerItem::new("a", "A").with_priority(TokenEncodingOptimizerPriority::High));
        m.add(TokenEncodingOptimizerItem::new("b", "B").with_priority(TokenEncodingOptimizerPriority::Low));
        m.add(TokenEncodingOptimizerItem::new("c", "C").with_priority(TokenEncodingOptimizerPriority::High));
        assert_eq!(m.by_priority(TokenEncodingOptimizerPriority::High).len(), 2);
    }

    #[test]
    fn tokenEncodingOptimizer_remove() {
        let mut m = TokenEncodingOptimizer::new();
        m.add(TokenEncodingOptimizerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn tokenEncodingOptimizer_search() {
        let mut m = TokenEncodingOptimizer::new();
        m.add(TokenEncodingOptimizerItem::new("id1", "Hello World"));
        m.add(TokenEncodingOptimizerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn tokenEncodingOptimizer_total_weight() {
        let mut m = TokenEncodingOptimizer::new();
        m.add(TokenEncodingOptimizerItem::new("a", "A").with_priority(TokenEncodingOptimizerPriority::Critical));
        m.add(TokenEncodingOptimizerItem::new("b", "B").with_priority(TokenEncodingOptimizerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn tokenEncodingOptimizer_capacity_limit() {
        let mut m = TokenEncodingOptimizer::new().with_max_items(2);
        m.add(TokenEncodingOptimizerItem::new("1", "one"));
        m.add(TokenEncodingOptimizerItem::new("2", "two"));
        assert!(!m.add(TokenEncodingOptimizerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn tokenEncodingOptimizer_sorted_by_priority() {
        let mut m = TokenEncodingOptimizer::new();
        m.add(TokenEncodingOptimizerItem::new("lo", "Low").with_priority(TokenEncodingOptimizerPriority::Low));
        m.add(TokenEncodingOptimizerItem::new("hi", "High").with_priority(TokenEncodingOptimizerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn tokenEncodingOptimizer_item_metadata() {
        let mut item = TokenEncodingOptimizerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn tokenPerformanceMonitor_enabled_toggle() {
        let mut s = TokenPerformanceMonitor::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn tokenEncodingOptimizer_priority_display() {
        assert_eq!(format!("{}", TokenEncodingOptimizerPriority::High), "high");
        assert_eq!(format!("{}", TokenEncodingOptimizerPriority::Low), "low");
    }


    #[test]
    fn tokens_x_config_new() {
        let c = TokensXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn tokens_x_config_builder() {
        let c = TokensXConfig::new("k")
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
    fn tokens_x_config_display() {
        let c = TokensXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn tokens_x_registry_insert_get() {
        let mut reg = TokensXRegistry::new();
        reg.insert(TokensXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn tokens_x_registry_duplicate() {
        let mut reg = TokensXRegistry::new();
        reg.insert(TokensXConfig::new("a")).unwrap();
        assert!(reg.insert(TokensXConfig::new("a")).is_err());
    }

    #[test]
    fn tokens_x_registry_remove() {
        let mut reg = TokensXRegistry::new();
        reg.insert(TokensXConfig::new("a")).unwrap();
        reg.insert(TokensXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn tokens_x_registry_active_entries() {
        let mut reg = TokensXRegistry::new();
        reg.insert(TokensXConfig::new("a")).unwrap();
        reg.insert(TokensXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn tokens_x_registry_by_weight() {
        let mut reg = TokensXRegistry::new();
        reg.insert(TokensXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(TokensXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn tokens_x_registry_tags() {
        let mut reg = TokensXRegistry::new();
        reg.insert(TokensXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(TokensXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn tokens_x_registry_total_weight() {
        let mut reg = TokensXRegistry::new();
        reg.insert(TokensXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(TokensXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn tokens_x_registry_iterator() {
        let mut reg = TokensXRegistry::new();
        reg.insert(TokensXConfig::new("a")).unwrap();
        reg.insert(TokensXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn tokens_x_cache_put_get() {
        let mut cache = TokensXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn tokens_x_cache_eviction() {
        let mut cache = TokensXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn tokens_x_cache_lru_order() {
        let mut cache = TokensXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn tokens_x_cache_most_least_recent() {
        let mut cache = TokensXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn tokens_x_formatter_entry() {
        let e = TokensXConfig::new("k").with_value("v");
        let fmt = TokensXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn tokens_x_formatter_summary() {
        let mut reg = TokensXRegistry::new();
        reg.insert(TokensXConfig::new("a").with_weight(5)).unwrap();
        let fmt = TokensXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn tokens_x_validator_valid() {
        let v = TokensXValidator::new();
        let c = TokensXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn tokens_x_validator_empty_key() {
        let v = TokensXValidator::new();
        let c = TokensXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn tokens_x_validator_require_value() {
        let v = TokensXValidator::new().require_value(true);
        let c = TokensXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn tokens_x_validator_allowed_tags() {
        let v = TokensXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = TokensXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn tokens_x_validator_validate_all() {
        let v = TokensXValidator::new();
        let mut reg = TokensXRegistry::new();
        reg.insert(TokensXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }

}
