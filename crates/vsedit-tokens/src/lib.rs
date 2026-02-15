//! Tokenization infrastructure.
//!
//! Equivalent to VS Code's `vs/editor/common/languages/supports/tokenization.ts`.
//! Provides token types and tokenization results used by syntax highlighting.

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
#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub start_offset: u32,
    pub metadata: TokenMetadata,
}

/// Result of tokenizing a line.
#[derive(Debug, Clone)]
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
}
