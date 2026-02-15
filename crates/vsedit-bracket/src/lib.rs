//! Bracket matching and colorization.

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
}
