//! Bracket matching and colorization.

use std::fmt;

/// A bracket pair definition.
#[derive(Debug, Clone)]
pub struct BracketPair {
    pub open: char,
    pub close: char,
}

/// Default bracket pairs.
pub fn default_bracket_pairs() -> Vec<BracketPair> {
    vec![
        BracketPair { open: '(', close: ')' },
        BracketPair { open: '[', close: ']' },
        BracketPair { open: '{', close: '}' },
    ]
}

/// A matched bracket position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketMatch {
    pub open_line: u32,
    pub open_col: u32,
    pub close_line: u32,
    pub close_col: u32,
    pub depth: u32,
}

/// Find the matching bracket for a position.
pub fn find_matching_bracket(
    lines: &[&str],
    line: u32,
    col: u32,
    pairs: &[BracketPair],
) -> Option<(u32, u32)> {
    let target_line = lines.get((line - 1) as usize)?;
    let ch = target_line.chars().nth((col - 1) as usize)?;

    // Check if it's an opening bracket
    if let Some(pair) = pairs.iter().find(|p| p.open == ch) {
        return find_closing(lines, line, col, pair);
    }

    // Check if it's a closing bracket
    if let Some(pair) = pairs.iter().find(|p| p.close == ch) {
        return find_opening(lines, line, col, pair);
    }

    None
}

fn find_closing(lines: &[&str], start_line: u32, start_col: u32, pair: &BracketPair) -> Option<(u32, u32)> {
    let mut depth: i32 = 0;
    for (li, line) in lines.iter().enumerate().skip((start_line - 1) as usize) {
        let start = if li == (start_line - 1) as usize { (start_col - 1) as usize } else { 0 };
        for (ci, ch) in line.char_indices().skip(start) {
            if ch == pair.open { depth += 1; }
            else if ch == pair.close {
                depth -= 1;
                if depth == 0 {
                    return Some(((li + 1) as u32, (ci + 1) as u32));
                }
            }
        }
    }
    None
}

fn find_opening(lines: &[&str], start_line: u32, start_col: u32, pair: &BracketPair) -> Option<(u32, u32)> {
    let mut depth: i32 = 0;
    for li in (0..start_line as usize).rev() {
        let line = lines[li];
        let end = if li == (start_line - 1) as usize { start_col as usize } else { line.len() };
        let chars: Vec<(usize, char)> = line.char_indices().take_while(|(i, _)| *i < end).collect();
        for &(ci, ch) in chars.iter().rev() {
            if ch == pair.close { depth += 1; }
            else if ch == pair.open {
                depth -= 1;
                if depth == 0 {
                    return Some(((li + 1) as u32, (ci + 1) as u32));
                }
            }
        }
    }
    None
}

/// Color index for bracket pair colorization (cycles through colors).
pub fn bracket_color_index(depth: u32, num_colors: u32) -> u32 {
    if num_colors == 0 { return 0; }
    depth % num_colors
}

/// Errors that can occur during bracket operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BracketError {
    /// The given position is outside the document bounds.
    InvalidPosition { line: u32, col: u32 },
    /// No bracket character found at the given position.
    NoBracketAtPosition { line: u32, col: u32 },
    /// A bracket was found but has no matching counterpart.
    UnmatchedBracket { line: u32, col: u32, ch: char },
}

impl fmt::Display for BracketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BracketError::InvalidPosition { line, col } => {
                write!(f, "invalid position: line {}, col {}", line, col)
            }
            BracketError::NoBracketAtPosition { line, col } => {
                write!(f, "no bracket at position: line {}, col {}", line, col)
            }
            BracketError::UnmatchedBracket { line, col, ch } => {
                write!(f, "unmatched bracket '{}' at line {}, col {}", ch, line, col)
            }
        }
    }
}

/// Configuration for bracket pair colorization and matching.
#[derive(Debug, Clone)]
pub struct BracketPairConfig {
    /// The bracket pairs to recognize.
    pub pairs: Vec<BracketPair>,
    /// Whether colorization is enabled.
    pub colorize_enabled: bool,
    /// Number of distinct colors to cycle through.
    pub num_colors: u32,
    /// Whether to highlight the active bracket pair.
    pub highlight_active: bool,
}

impl Default for BracketPairConfig {
    fn default() -> Self {
        Self {
            pairs: default_bracket_pairs(),
            colorize_enabled: true,
            num_colors: 6,
            highlight_active: true,
        }
    }
}

impl fmt::Display for BracketPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.open, self.close)
    }
}

/// Find all matched bracket pairs in a document.
///
/// Returns a list of `BracketMatch` entries for every properly matched pair.
pub fn find_all_brackets(lines: &[&str], pairs: &[BracketPair]) -> Vec<BracketMatch> {
    let mut results = Vec::new();
    // One stack per pair type to handle interleaved bracket kinds.
    let mut stacks: Vec<Vec<(u32, u32)>> = vec![Vec::new(); pairs.len()];

    for (li, line) in lines.iter().enumerate() {
        for (ci, ch) in line.char_indices() {
            let line1 = (li + 1) as u32;
            let col1 = (ci + 1) as u32;

            for (pi, pair) in pairs.iter().enumerate() {
                if ch == pair.open {
                    stacks[pi].push((line1, col1));
                } else if ch == pair.close {
                    if let Some((open_line, open_col)) = stacks[pi].pop() {
                        let depth = stacks[pi].len() as u32;
                        results.push(BracketMatch {
                            open_line,
                            open_col,
                            close_line: line1,
                            close_col: col1,
                            depth,
                        });
                    }
                }
            }
        }
    }
    results
}

/// Validate that all brackets in a document are properly matched.
///
/// Returns `Ok(())` if every opening bracket has a corresponding closing bracket,
/// or the first `BracketError::UnmatchedBracket` found.
pub fn validate_brackets(lines: &[&str], pairs: &[BracketPair]) -> Result<(), BracketError> {
    let mut stacks: Vec<Vec<(u32, u32, char)>> = vec![Vec::new(); pairs.len()];

    for (li, line) in lines.iter().enumerate() {
        for (ci, ch) in line.char_indices() {
            let line1 = (li + 1) as u32;
            let col1 = (ci + 1) as u32;

            for (pi, pair) in pairs.iter().enumerate() {
                if ch == pair.open {
                    stacks[pi].push((line1, col1, ch));
                } else if ch == pair.close {
                    if stacks[pi].pop().is_none() {
                        return Err(BracketError::UnmatchedBracket {
                            line: line1,
                            col: col1,
                            ch,
                        });
                    }
                }
            }
        }
    }

    // Check for any unclosed opening brackets.
    for stack in &stacks {
        if let Some(&(line, col, ch)) = stack.first() {
            return Err(BracketError::UnmatchedBracket { line, col, ch });
        }
    }

    Ok(())
}

/// Find the nearest enclosing bracket pair around a position.
///
/// Searches outward from `(line, col)` to find the innermost bracket pair
/// that contains the position.
pub fn find_enclosing_brackets(
    lines: &[&str],
    line: u32,
    col: u32,
    pairs: &[BracketPair],
) -> Result<BracketMatch, BracketError> {
    if line == 0 || line as usize > lines.len() {
        return Err(BracketError::InvalidPosition { line, col });
    }
    let target_line = lines[(line - 1) as usize];
    if col == 0 || col as usize > target_line.len() {
        return Err(BracketError::InvalidPosition { line, col });
    }

    let all = find_all_brackets(lines, pairs);

    // Filter to pairs that enclose the position, pick the tightest one.
    let mut best: Option<&BracketMatch> = None;
    for m in &all {
        let after_open =
            m.open_line < line || (m.open_line == line && m.open_col < col);
        let before_close =
            m.close_line > line || (m.close_line == line && m.close_col > col);

        if after_open && before_close {
            match best {
                None => best = Some(m),
                Some(prev) => {
                    let prev_start = (prev.open_line, prev.open_col);
                    let cur_start = (m.open_line, m.open_col);
                    // Tighter means opened later.
                    if cur_start > prev_start {
                        best = Some(m);
                    }
                }
            }
        }
    }

    best.cloned().ok_or(BracketError::NoBracketAtPosition { line, col })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_forward() {
        let lines = vec!["fn f() {", "  x", "}"];
        let pairs = default_bracket_pairs();
        let result = find_matching_bracket(&lines, 1, 8, &pairs);
        assert_eq!(result, Some((3, 1)));
    }

    #[test]
    fn match_backward() {
        let lines = vec!["fn f() {", "  x", "}"];
        let pairs = default_bracket_pairs();
        let result = find_matching_bracket(&lines, 3, 1, &pairs);
        assert_eq!(result, Some((1, 8)));
    }

    #[test]
    fn match_parens() {
        let lines = vec!["(a + (b * c))"];
        let pairs = default_bracket_pairs();
        assert_eq!(find_matching_bracket(&lines, 1, 1, &pairs), Some((1, 13)));
        assert_eq!(find_matching_bracket(&lines, 1, 6, &pairs), Some((1, 12)));
    }

    #[test]
    fn no_match() {
        let lines = vec!["(unclosed"];
        let pairs = default_bracket_pairs();
        assert_eq!(find_matching_bracket(&lines, 1, 1, &pairs), None);
    }

    #[test]
    fn color_cycling() {
        assert_eq!(bracket_color_index(0, 6), 0);
        assert_eq!(bracket_color_index(5, 6), 5);
        assert_eq!(bracket_color_index(6, 6), 0);
    }

    #[test]
    fn color_cycling_zero_colors() {
        assert_eq!(bracket_color_index(3, 0), 0);
    }

    #[test]
    fn bracket_error_display() {
        let e1 = BracketError::InvalidPosition { line: 1, col: 5 };
        assert_eq!(e1.to_string(), "invalid position: line 1, col 5");

        let e2 = BracketError::NoBracketAtPosition { line: 2, col: 3 };
        assert_eq!(e2.to_string(), "no bracket at position: line 2, col 3");

        let e3 = BracketError::UnmatchedBracket { line: 1, col: 1, ch: '(' };
        assert_eq!(e3.to_string(), "unmatched bracket '(' at line 1, col 1");
    }

    #[test]
    fn bracket_pair_display() {
        let pair = BracketPair { open: '(', close: ')' };
        assert_eq!(pair.to_string(), "()");
        let pair2 = BracketPair { open: '{', close: '}' };
        assert_eq!(pair2.to_string(), "{}");
    }

    #[test]
    fn bracket_pair_config_default() {
        let config = BracketPairConfig::default();
        assert_eq!(config.pairs.len(), 3);
        assert!(config.colorize_enabled);
        assert_eq!(config.num_colors, 6);
        assert!(config.highlight_active);
    }

    #[test]
    fn find_all_brackets_simple() {
        let lines = vec!["(a + b)"];
        let pairs = default_bracket_pairs();
        let matches = find_all_brackets(&lines, &pairs);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].open_line, 1);
        assert_eq!(matches[0].open_col, 1);
        assert_eq!(matches[0].close_line, 1);
        assert_eq!(matches[0].close_col, 7);
        assert_eq!(matches[0].depth, 0);
    }

    #[test]
    fn find_all_brackets_nested() {
        let lines = vec!["((a))"];
        let pairs = default_bracket_pairs();
        let matches = find_all_brackets(&lines, &pairs);
        assert_eq!(matches.len(), 2);
        // Inner pair is matched first (closed first).
        let inner = &matches[0];
        assert_eq!(inner.open_line, 1);
        assert_eq!(inner.open_col, 2);
        assert_eq!(inner.depth, 1);
        let outer = &matches[1];
        assert_eq!(outer.open_col, 1);
        assert_eq!(outer.close_col, 5);
    }

    #[test]
    fn find_all_brackets_multiline() {
        let lines = vec!["fn f() {", "  (x)", "}"];
        let pairs = default_bracket_pairs();
        let matches = find_all_brackets(&lines, &pairs);
        // Should find: parens around args, parens around x, braces
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn validate_brackets_ok() {
        let lines = vec!["fn f() { [x] }"];
        let pairs = default_bracket_pairs();
        assert!(validate_brackets(&lines, &pairs).is_ok());
    }

    #[test]
    fn validate_brackets_unclosed() {
        let lines = vec!["(a + b"];
        let pairs = default_bracket_pairs();
        let err = validate_brackets(&lines, &pairs).unwrap_err();
        assert_eq!(err, BracketError::UnmatchedBracket { line: 1, col: 1, ch: '(' });
    }

    #[test]
    fn validate_brackets_extra_close() {
        let lines = vec!["a + b)"];
        let pairs = default_bracket_pairs();
        let err = validate_brackets(&lines, &pairs).unwrap_err();
        assert_eq!(err, BracketError::UnmatchedBracket { line: 1, col: 6, ch: ')' });
    }

    #[test]
    fn find_enclosing_simple() {
        let lines = vec!["(hello)"];
        let pairs = default_bracket_pairs();
        let m = find_enclosing_brackets(&lines, 1, 3, &pairs).unwrap();
        assert_eq!(m.open_line, 1);
        assert_eq!(m.open_col, 1);
        assert_eq!(m.close_col, 7);
    }

    #[test]
    fn find_enclosing_invalid_position() {
        let lines = vec!["abc"];
        let pairs = default_bracket_pairs();
        let err = find_enclosing_brackets(&lines, 0, 1, &pairs).unwrap_err();
        assert_eq!(err, BracketError::InvalidPosition { line: 0, col: 1 });
    }

    #[test]
    fn find_enclosing_no_brackets() {
        let lines = vec!["hello world"];
        let pairs = default_bracket_pairs();
        let err = find_enclosing_brackets(&lines, 1, 3, &pairs).unwrap_err();
        assert_eq!(err, BracketError::NoBracketAtPosition { line: 1, col: 3 });
    }
}
