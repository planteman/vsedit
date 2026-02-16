//! Unicode confusable detection.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicodeCategory {
    Ambiguous,
    Invisible,
    NonBasicAscii,
    ConfusableWithAscii,
}

#[derive(Debug, Clone)]
pub struct UnicodeHighlight {
    pub line: u32,
    pub column: u32,
    pub character: char,
    pub category: UnicodeCategory,
    pub replacement: Option<char>,
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
}
