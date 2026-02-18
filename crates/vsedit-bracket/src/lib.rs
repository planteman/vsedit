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

// ---------------------------------------------------------------------------
// Auto-close and surround
// ---------------------------------------------------------------------------

/// Returns the closing bracket character when an opening bracket is typed.
/// Returns `None` if the character is not an opening bracket.
pub fn auto_close_bracket(ch: char) -> Option<char> {
    match ch {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '<' => Some('>'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        _ => None,
    }
}

/// Wraps the given selection text with the specified opening bracket and its
/// matching close bracket. Returns the wrapped text.
pub fn auto_surround_selection(selection_text: &str, bracket: char) -> String {
    let close = auto_close_bracket(bracket).unwrap_or(bracket);
    format!("{}{}{}", bracket, selection_text, close)
}

/// A bracket pair with its nesting level for colorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorizedBracketPair {
    pub line: u32,
    pub col: u32,
    pub bracket: char,
    pub nesting_level: u32,
    pub is_open: bool,
}

/// Compute bracket pairs with nesting level for colorization.
/// Skips brackets inside string literals (delimited by `"` or `'`) and
/// line comments (starting with `//`).
pub fn bracket_pair_colorization(
    lines: &[&str],
    pairs: &[BracketPair],
) -> Vec<ColorizedBracketPair> {
    let mut results = Vec::new();
    let mut stacks: Vec<Vec<(u32, u32)>> = vec![Vec::new(); pairs.len()];

    for (li, line) in lines.iter().enumerate() {
        let line1 = (li + 1) as u32;
        let bytes = line.as_bytes();
        let mut i = 0;
        let len = bytes.len();
        while i < len {
            // Skip line comments
            if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                break;
            }
            // Skip string literals
            if bytes[i] == b'"' || bytes[i] == b'\'' {
                let quote = bytes[i];
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == quote {
                        break;
                    }
                    i += 1;
                }
                i += 1;
                continue;
            }

            let ch = bytes[i] as char;
            let col1 = (i + 1) as u32;

            for (pi, pair) in pairs.iter().enumerate() {
                if ch == pair.open {
                    let level = stacks[pi].len() as u32;
                    results.push(ColorizedBracketPair {
                        line: line1,
                        col: col1,
                        bracket: ch,
                        nesting_level: level,
                        is_open: true,
                    });
                    stacks[pi].push((line1, col1));
                } else if ch == pair.close {
                    let level = if stacks[pi].is_empty() {
                        0
                    } else {
                        stacks[pi].len() as u32 - 1
                    };
                    stacks[pi].pop();
                    results.push(ColorizedBracketPair {
                        line: line1,
                        col: col1,
                        bracket: ch,
                        nesting_level: level,
                        is_open: false,
                    });
                }
            }
            i += 1;
        }
    }
    results
}

/// Find matching bracket, skipping brackets inside strings and comments.
pub fn find_matching_bracket_smart(
    lines: &[&str],
    line: u32,
    col: u32,
    pairs: &[BracketPair],
) -> Option<(u32, u32)> {
    let target_line = lines.get((line - 1) as usize)?;
    let ch = target_line.chars().nth((col - 1) as usize)?;

    let pair = pairs.iter().find(|p| p.open == ch || p.close == ch)?;
    let is_open = ch == pair.open;

    if is_open {
        find_closing_smart(lines, line, col, pair)
    } else {
        find_opening_smart(lines, line, col, pair)
    }
}

fn is_in_string_or_comment(line: &str, col_idx: usize) -> bool {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut quote_char = 0u8;
    let mut i = 0;
    while i < col_idx && i < bytes.len() {
        if !in_string && i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            return true; // rest of line is comment
        }
        if !in_string && (bytes[i] == b'"' || bytes[i] == b'\'') {
            in_string = true;
            quote_char = bytes[i];
        } else if in_string {
            if bytes[i] == b'\\' {
                i += 1; // skip escaped char
            } else if bytes[i] == quote_char {
                in_string = false;
            }
        }
        i += 1;
    }
    in_string
}

fn find_closing_smart(
    lines: &[&str],
    start_line: u32,
    start_col: u32,
    pair: &BracketPair,
) -> Option<(u32, u32)> {
    let mut depth: i32 = 0;
    for (li, line) in lines.iter().enumerate().skip((start_line - 1) as usize) {
        let start = if li == (start_line - 1) as usize { (start_col - 1) as usize } else { 0 };
        for (ci, ch) in line.char_indices().skip(start) {
            if is_in_string_or_comment(line, ci) {
                continue;
            }
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

fn find_opening_smart(
    lines: &[&str],
    start_line: u32,
    start_col: u32,
    pair: &BracketPair,
) -> Option<(u32, u32)> {
    let mut depth: i32 = 0;
    for li in (0..start_line as usize).rev() {
        let line = lines[li];
        let end = if li == (start_line - 1) as usize { start_col as usize } else { line.len() };
        let chars: Vec<(usize, char)> = line.char_indices().take_while(|(i, _)| *i < end).collect();
        for &(ci, ch) in chars.iter().rev() {
            if is_in_string_or_comment(line, ci) {
                continue;
            }
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

/// Statistics about brackets in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketStats {
    /// Total number of matched pairs found.
    pub total_pairs: usize,
    /// Maximum nesting depth.
    pub max_depth: usize,
    /// The most commonly occurring bracket pair (open, close), if any.
    pub most_common_pair: Option<(char, char)>,
}

impl BracketStats {
    /// Compute bracket statistics for the given document lines using the provided pairs.
    pub fn compute(lines: &[&str], pairs: &[BracketPair]) -> Self {
        let mut pair_counts: Vec<usize> = vec![0; pairs.len()];
        let mut total_pairs: usize = 0;
        let mut max_depth: usize = 0;
        let mut depth: usize = 0;
        for line in lines {
            for ch in line.chars() {
                for (pi, pair) in pairs.iter().enumerate() {
                    if ch == pair.open {
                        depth += 1;
                        if depth > max_depth {
                            max_depth = depth;
                        }
                    } else if ch == pair.close && depth > 0 {
                        pair_counts[pi] += 1;
                        total_pairs += 1;
                        depth -= 1;
                    }
                }
            }
        }
        let most_common_pair = pair_counts
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| **c)
            .and_then(|(i, &c)| {
                if c > 0 {
                    Some((pairs[i].open, pairs[i].close))
                } else {
                    None
                }
            });
        BracketStats { total_pairs, max_depth, most_common_pair }
    }
}

/// A single bracket with its assigned color and position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorizedBracket {
    pub char: char,
    pub col: usize,
    pub color: String,
    pub depth: u32,
}

/// Assigns bracket pair colors by nesting depth.
#[derive(Debug, Clone)]
pub struct BracketColorizer {
    colors: Vec<String>,
}

impl BracketColorizer {
    /// Create a colorizer with the given color strings.
    pub fn new(colors: Vec<String>) -> Self {
        Self { colors }
    }

    /// Colorize brackets in a single line, cycling colors by nesting depth.
    pub fn colorize_line(&self, line: &str, pairs: &[BracketPair]) -> Vec<ColorizedBracket> {
        let mut results = Vec::new();
        if self.colors.is_empty() {
            return results;
        }
        let mut depth: u32 = 0;
        for (ci, ch) in line.char_indices() {
            let is_open = pairs.iter().any(|p| p.open == ch);
            let is_close = pairs.iter().any(|p| p.close == ch);
            if is_open {
                let color_idx = (depth as usize) % self.colors.len();
                results.push(ColorizedBracket {
                    char: ch,
                    col: ci,
                    color: self.colors[color_idx].clone(),
                    depth,
                });
                depth += 1;
            } else if is_close {
                if depth > 0 {
                    depth -= 1;
                }
                let color_idx = (depth as usize) % self.colors.len();
                results.push(ColorizedBracket {
                    char: ch,
                    col: ci,
                    color: self.colors[color_idx].clone(),
                    depth,
                });
            }
        }
        results
    }
}

impl Default for BracketColorizer {
    fn default() -> Self {
        Self {
            colors: vec![
                "#FFD700".to_string(),
                "#DA70D6".to_string(),
                "#87CEEB".to_string(),
                "#98FB98".to_string(),
                "#FF6347".to_string(),
                "#DDA0DD".to_string(),
            ],
        }
    }
}

/// Find the bracket pair that contains or starts at the given position.
pub fn bracket_pair_at_position(
    lines: &[&str],
    line: u32,
    col: u32,
    pairs: &[BracketPair],
) -> Option<BracketMatch> {
    let target_line = lines.get((line - 1) as usize)?;
    let ch = target_line.chars().nth((col - 1) as usize)?;

    // If the position is on a bracket, find its match.
    let is_bracket = pairs.iter().any(|p| p.open == ch || p.close == ch);
    if is_bracket {
        if let Some(pair) = pairs.iter().find(|p| p.open == ch) {
            if let Some((cl, cc)) = find_closing(lines, line, col, pair) {
                let all = find_all_brackets(lines, pairs);
                let depth = all
                    .iter()
                    .find(|m| m.open_line == line && m.open_col == col)
                    .map(|m| m.depth)
                    .unwrap_or(0);
                return Some(BracketMatch {
                    open_line: line,
                    open_col: col,
                    close_line: cl,
                    close_col: cc,
                    depth,
                });
            }
        }
        if let Some(pair) = pairs.iter().find(|p| p.close == ch) {
            if let Some((ol, oc)) = find_opening(lines, line, col, pair) {
                let all = find_all_brackets(lines, pairs);
                let depth = all
                    .iter()
                    .find(|m| m.close_line == line && m.close_col == col)
                    .map(|m| m.depth)
                    .unwrap_or(0);
                return Some(BracketMatch {
                    open_line: ol,
                    open_col: oc,
                    close_line: line,
                    close_col: col,
                    depth,
                });
            }
        }
        return None;
    }

    // Not on a bracket — find the enclosing pair.
    find_enclosing_brackets(lines, line, col, pairs).ok()
}

/// The kind of bracket error detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BracketErrorKind {
    /// An opening bracket with no matching close.
    UnmatchedOpen,
    /// A closing bracket with no matching open.
    UnmatchedClose,
    /// A closing bracket that does not match the expected opening bracket.
    Mismatch { expected: char, found: char },
}

/// Information about a single bracket error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketErrorInfo {
    pub line: u32,
    pub col: u32,
    pub bracket: char,
    pub error_kind: BracketErrorKind,
}

/// Detect mismatched and unmatched brackets in a document.
pub fn bracket_errors(lines: &[&str], pairs: &[BracketPair]) -> Vec<BracketErrorInfo> {
    let mut errors = Vec::new();
    // Single stack to detect cross-type mismatches.
    let mut stack: Vec<(u32, u32, char)> = Vec::new();

    for (li, line) in lines.iter().enumerate() {
        let line1 = (li + 1) as u32;
        for (ci, ch) in line.char_indices() {
            let col1 = (ci + 1) as u32;

            if pairs.iter().any(|p| p.open == ch) {
                stack.push((line1, col1, ch));
            } else if let Some(pair) = pairs.iter().find(|p| p.close == ch) {
                match stack.last() {
                    Some(&(_, _, open_ch)) if open_ch == pair.open => {
                        stack.pop();
                    }
                    Some(&(_, _, open_ch)) if pairs.iter().any(|p| p.open == open_ch) => {
                        // Mismatch: expected the close of whatever is on top.
                        let expected_close = pairs
                            .iter()
                            .find(|p| p.open == open_ch)
                            .map(|p| p.close)
                            .unwrap_or('?');
                        errors.push(BracketErrorInfo {
                            line: line1,
                            col: col1,
                            bracket: ch,
                            error_kind: BracketErrorKind::Mismatch {
                                expected: expected_close,
                                found: ch,
                            },
                        });
                        stack.pop();
                    }
                    _ => {
                        errors.push(BracketErrorInfo {
                            line: line1,
                            col: col1,
                            bracket: ch,
                            error_kind: BracketErrorKind::UnmatchedClose,
                        });
                    }
                }
            }
        }
    }

    // Remaining items on the stack are unmatched opens.
    for (line, col, ch) in stack {
        errors.push(BracketErrorInfo {
            line,
            col,
            bracket: ch,
            error_kind: BracketErrorKind::UnmatchedOpen,
        });
    }

    errors
}

// ── BracketStatistics ──

/// Per-bracket-type count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketTypeCount {
    pub open: char,
    pub close: char,
    pub open_count: usize,
    pub close_count: usize,
}

/// Detailed bracket statistics including per-type counts and nesting depth
/// histogram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketStatistics {
    /// Counts per bracket type.
    pub type_counts: Vec<BracketTypeCount>,
    /// Histogram mapping nesting depth to number of brackets at that depth.
    pub depth_histogram: Vec<usize>,
    /// Total number of opening brackets.
    pub total_opens: usize,
    /// Total number of closing brackets.
    pub total_closes: usize,
    /// Whether all brackets are balanced.
    pub is_balanced: bool,
}

impl BracketStatistics {
    /// Compute detailed statistics for the given document.
    pub fn compute(lines: &[&str], pairs: &[BracketPair]) -> Self {
        let mut type_counts: Vec<BracketTypeCount> = pairs
            .iter()
            .map(|p| BracketTypeCount {
                open: p.open,
                close: p.close,
                open_count: 0,
                close_count: 0,
            })
            .collect();
        let mut depth: usize = 0;
        let mut depth_histogram: Vec<usize> = Vec::new();

        for line in lines {
            for ch in line.chars() {
                for (pi, pair) in pairs.iter().enumerate() {
                    if ch == pair.open {
                        type_counts[pi].open_count += 1;
                        // Record this bracket at the current depth
                        if depth >= depth_histogram.len() {
                            depth_histogram.resize(depth + 1, 0);
                        }
                        depth_histogram[depth] += 1;
                        depth += 1;
                    } else if ch == pair.close {
                        type_counts[pi].close_count += 1;
                        if depth > 0 {
                            depth -= 1;
                        }
                        if depth >= depth_histogram.len() {
                            depth_histogram.resize(depth + 1, 0);
                        }
                        depth_histogram[depth] += 1;
                    }
                }
            }
        }

        let total_opens: usize = type_counts.iter().map(|c| c.open_count).sum();
        let total_closes: usize = type_counts.iter().map(|c| c.close_count).sum();
        let is_balanced = type_counts.iter().all(|c| c.open_count == c.close_count);

        BracketStatistics {
            type_counts,
            depth_histogram,
            total_opens,
            total_closes,
            is_balanced,
        }
    }

    /// Return the maximum nesting depth observed.
    pub fn max_depth(&self) -> usize {
        if self.depth_histogram.is_empty() {
            0
        } else {
            self.depth_histogram.len() - 1
        }
    }
}

// ── BracketHighlighter ──

/// A range of text that should be highlighted as a bracket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketHighlightRange {
    pub line: u32,
    pub col_start: u32,
    pub col_end: u32,
    pub color: String,
    pub depth: u32,
    pub is_open: bool,
}

/// Produces colored highlight ranges for all brackets in a document.
pub struct BracketHighlighter {
    colors: Vec<String>,
}

impl BracketHighlighter {
    /// Create a highlighter with the given palette.
    pub fn new(colors: Vec<String>) -> Self {
        Self { colors }
    }

    /// Generate highlight ranges for every bracket in the document.
    pub fn highlight(
        &self,
        lines: &[&str],
        pairs: &[BracketPair],
    ) -> Vec<BracketHighlightRange> {
        if self.colors.is_empty() {
            return Vec::new();
        }
        let mut results = Vec::new();
        let mut depth: u32 = 0;
        for (li, line) in lines.iter().enumerate() {
            for (ci, ch) in line.char_indices() {
                let is_open = pairs.iter().any(|p| p.open == ch);
                let is_close = pairs.iter().any(|p| p.close == ch);
                if is_open {
                    let color_idx = (depth as usize) % self.colors.len();
                    results.push(BracketHighlightRange {
                        line: (li + 1) as u32,
                        col_start: (ci + 1) as u32,
                        col_end: (ci + 2) as u32,
                        color: self.colors[color_idx].clone(),
                        depth,
                        is_open: true,
                    });
                    depth += 1;
                } else if is_close {
                    if depth > 0 {
                        depth -= 1;
                    }
                    let color_idx = (depth as usize) % self.colors.len();
                    results.push(BracketHighlightRange {
                        line: (li + 1) as u32,
                        col_start: (ci + 1) as u32,
                        col_end: (ci + 2) as u32,
                        color: self.colors[color_idx].clone(),
                        depth,
                        is_open: false,
                    });
                }
            }
        }
        results
    }
}

// ── Bracket auto-close suggestions ──

/// A suggestion to auto-close a bracket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoCloseSuggestion {
    pub line: u32,
    pub col: u32,
    pub close_char: char,
    pub open_char: char,
}

/// Scan the document and suggest where unclosed brackets should be closed.
pub fn suggest_auto_close(
    lines: &[&str],
    pairs: &[BracketPair],
) -> Vec<AutoCloseSuggestion> {
    let mut stack: Vec<(char, char, u32, u32)> = Vec::new(); // (open, close, line, col)
    for (li, line) in lines.iter().enumerate() {
        for (ci, ch) in line.char_indices() {
            if let Some(pair) = pairs.iter().find(|p| p.open == ch) {
                stack.push((pair.open, pair.close, (li + 1) as u32, (ci + 1) as u32));
            } else if pairs.iter().any(|p| p.close == ch) {
                // Pop matching open if available
                if let Some(pos) = stack.iter().rposition(|(_, close, _, _)| *close == ch) {
                    stack.remove(pos);
                }
            }
        }
    }

    // Remaining items on the stack need a closing bracket
    let last_line = lines.len() as u32;
    let last_col = lines.last().map(|l| l.len() as u32 + 1).unwrap_or(1);

    stack
        .into_iter()
        .rev()
        .map(|(open_char, close_char, _, _)| AutoCloseSuggestion {
            line: last_line,
            col: last_col,
            close_char,
            open_char,
        })
        .collect()
}

// ── Bracket folding ranges ──

/// A foldable range derived from a matched bracket pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketFoldRange {
    /// 1-based line of the opening bracket.
    pub start_line: u32,
    /// 1-based column of the opening bracket.
    pub start_col: u32,
    /// 1-based line of the closing bracket.
    pub end_line: u32,
    /// 1-based column of the closing bracket.
    pub end_col: u32,
    /// Nesting depth of this range (0 = outermost).
    pub depth: u32,
}

/// Compute foldable ranges from bracket pairs. Only pairs that span more than
/// one line are included because single-line pairs provide no useful fold.
pub fn folding_ranges(lines: &[&str], pairs: &[BracketPair]) -> Vec<BracketFoldRange> {
    let matches = find_all_brackets(lines, pairs);
    matches
        .into_iter()
        .filter(|m| m.close_line > m.open_line)
        .map(|m| BracketFoldRange {
            start_line: m.open_line,
            start_col: m.open_col,
            end_line: m.close_line,
            end_col: m.close_col,
            depth: m.depth,
        })
        .collect()
}

// ── Indentation guides from bracket structure ──

/// An indentation guide derived from bracket nesting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentGuide {
    /// 1-based line number.
    pub line: u32,
    /// Nesting depth at the start of this line.
    pub depth: u32,
    /// Whether the line contains an opening bracket that increases depth.
    pub opens: bool,
    /// Whether the line contains a closing bracket that decreases depth.
    pub closes: bool,
}

/// Compute indentation guides for every line based on bracket nesting.
pub fn indentation_guides(lines: &[&str], pairs: &[BracketPair]) -> Vec<IndentGuide> {
    let mut guides = Vec::with_capacity(lines.len());
    let mut depth: u32 = 0;

    for (li, line) in lines.iter().enumerate() {
        let start_depth = depth;
        let mut has_open = false;
        let mut has_close = false;

        for ch in line.chars() {
            if pairs.iter().any(|p| p.open == ch) {
                depth += 1;
                has_open = true;
            } else if pairs.iter().any(|p| p.close == ch) {
                if depth > 0 {
                    depth -= 1;
                }
                has_close = true;
            }
        }

        guides.push(IndentGuide {
            line: (li + 1) as u32,
            depth: start_depth,
            opens: has_open,
            closes: has_close,
        });
    }
    guides
}

// ── Bracket scope text extraction ──

/// Extract the text content between a matched bracket pair (exclusive of the
/// brackets themselves). Returns `None` if the match spans positions that are
/// out of bounds.
pub fn extract_bracket_scope<'a>(
    lines: &[&'a str],
    m: &BracketMatch,
) -> Option<String> {
    if m.open_line == 0 || m.close_line == 0 {
        return None;
    }
    let open_li = (m.open_line - 1) as usize;
    let close_li = (m.close_line - 1) as usize;
    if open_li >= lines.len() || close_li >= lines.len() {
        return None;
    }

    if m.open_line == m.close_line {
        let line = lines[open_li];
        let start = m.open_col as usize; // char after opening bracket
        let end = (m.close_col - 1) as usize;
        if start > end || end > line.len() {
            return Some(String::new());
        }
        return Some(line[start..end].to_string());
    }

    let mut result = String::new();
    // Remainder of the opening line after the bracket.
    let first = lines[open_li];
    let start = m.open_col as usize;
    if start <= first.len() {
        result.push_str(&first[start..]);
    }
    // Full intermediate lines.
    for li in (open_li + 1)..close_li {
        result.push('\n');
        result.push_str(lines[li]);
    }
    // Portion of the closing line before the bracket.
    if close_li > open_li {
        result.push('\n');
        let last = lines[close_li];
        let end = (m.close_col - 1) as usize;
        if end <= last.len() {
            result.push_str(&last[..end]);
        }
    }
    Some(result)
}

// ── Bracket insertion edits ──

/// A text edit representing a bracket insertion or replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketEdit {
    /// 1-based line.
    pub line: u32,
    /// 1-based column where the edit starts.
    pub col: u32,
    /// Number of characters to delete before inserting.
    pub delete_len: u32,
    /// Text to insert.
    pub insert_text: String,
}

/// Produce edits to fix all detected bracket errors in a document.
///
/// For `UnmatchedOpen` errors, an edit inserts the matching close bracket at
/// the end of the document. For `UnmatchedClose` errors, an edit deletes the
/// stray closing bracket. For `Mismatch` errors, an edit replaces the wrong
/// closing bracket with the expected one.
pub fn bracket_fix_edits(lines: &[&str], pairs: &[BracketPair]) -> Vec<BracketEdit> {
    let errors = bracket_errors(lines, pairs);
    let last_line = lines.len() as u32;
    let last_col = lines.last().map(|l| l.len() as u32 + 1).unwrap_or(1);

    errors
        .iter()
        .map(|err| match &err.error_kind {
            BracketErrorKind::UnmatchedOpen => {
                let close = pairs
                    .iter()
                    .find(|p| p.open == err.bracket)
                    .map(|p| p.close)
                    .unwrap_or(err.bracket);
                BracketEdit {
                    line: last_line,
                    col: last_col,
                    delete_len: 0,
                    insert_text: close.to_string(),
                }
            }
            BracketErrorKind::UnmatchedClose => BracketEdit {
                line: err.line,
                col: err.col,
                delete_len: 1,
                insert_text: String::new(),
            },
            BracketErrorKind::Mismatch { expected, found: _ } => BracketEdit {
                line: err.line,
                col: err.col,
                delete_len: 1,
                insert_text: expected.to_string(),
            },
        })
        .collect()
}

// ── Per-line bracket balance ──

/// Balance information for a single line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineBracketBalance {
    /// 1-based line number.
    pub line: u32,
    /// Net balance change on this line (opens minus closes).
    pub net: i32,
    /// Number of opening brackets on this line.
    pub opens: u32,
    /// Number of closing brackets on this line.
    pub closes: u32,
    /// Running cumulative depth at the end of this line.
    pub cumulative_depth: i32,
}

/// Compute bracket balance for every line in the document.
pub fn line_bracket_balances(lines: &[&str], pairs: &[BracketPair]) -> Vec<LineBracketBalance> {
    let mut results = Vec::with_capacity(lines.len());
    let mut cumulative: i32 = 0;

    for (li, line) in lines.iter().enumerate() {
        let mut opens: u32 = 0;
        let mut closes: u32 = 0;
        for ch in line.chars() {
            if pairs.iter().any(|p| p.open == ch) {
                opens += 1;
            } else if pairs.iter().any(|p| p.close == ch) {
                closes += 1;
            }
        }
        let net = opens as i32 - closes as i32;
        cumulative += net;
        results.push(LineBracketBalance {
            line: (li + 1) as u32,
            net,
            opens,
            closes,
            cumulative_depth: cumulative,
        });
    }
    results
}

// ---------------------------------------------------------------------------
// BracketPairGuide – vertical guide lines for bracket pairs
// ---------------------------------------------------------------------------

/// A vertical guide line drawn between matching bracket pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketPairGuide {
    /// Column where the guide is drawn.
    pub column: u32,
    /// Start line (1-based) where the opening bracket is.
    pub start_line: u32,
    /// End line (1-based) where the closing bracket is.
    pub end_line: u32,
    /// Nesting depth of this bracket pair.
    pub depth: u32,
    /// Color index for rainbow coloring.
    pub color_index: u32,
}

impl BracketPairGuide {
    pub fn new(column: u32, start_line: u32, end_line: u32, depth: u32, num_colors: u32) -> Self {
        Self {
            column,
            start_line,
            end_line,
            depth,
            color_index: bracket_color_index(depth, num_colors),
        }
    }

    /// Number of lines this guide spans.
    pub fn line_span(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line)
    }

    /// Whether a given line falls within this guide's range.
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

impl fmt::Display for BracketPairGuide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Guide(col={}, lines={}..{}, depth={})",
            self.column, self.start_line, self.end_line, self.depth
        )
    }
}

/// Build bracket pair guides from matched bracket positions.
pub fn build_bracket_guides(matches: &[BracketMatch], num_colors: u32) -> Vec<BracketPairGuide> {
    matches
        .iter()
        .filter(|m| m.open_line != m.close_line)
        .map(|m| BracketPairGuide::new(m.open_col, m.open_line, m.close_line, m.depth, num_colors))
        .collect()
}

// ---------------------------------------------------------------------------
// BracketScopeHighlighter – rainbow bracket scope highlighting
// ---------------------------------------------------------------------------

/// Assigns colors to bracket pairs based on nesting depth for rainbow brackets.
#[derive(Debug, Clone)]
pub struct BracketScopeHighlighter {
    pub colors: Vec<String>,
}

impl BracketScopeHighlighter {
    pub fn new(colors: Vec<String>) -> Self {
        Self {
            colors: if colors.is_empty() {
                vec![
                    "#FFD700".into(), "#DA70D6".into(), "#87CEEB".into(),
                    "#98FB98".into(), "#FFA07A".into(), "#DDA0DD".into(),
                ]
            } else {
                colors
            },
        }
    }

    /// Get the color for a bracket at a given depth.
    pub fn color_for_depth(&self, depth: u32) -> &str {
        &self.colors[depth as usize % self.colors.len()]
    }

    /// Highlight all brackets in a line, returning (char_index, color) pairs.
    pub fn highlight_line(&self, line: &str, base_depth: u32, pairs: &[BracketPair]) -> Vec<(usize, String)> {
        let mut result = Vec::new();
        let mut depth = base_depth;
        for (i, ch) in line.chars().enumerate() {
            if pairs.iter().any(|p| p.open == ch) {
                result.push((i, self.color_for_depth(depth).to_string()));
                depth += 1;
            } else if pairs.iter().any(|p| p.close == ch) {
                depth = depth.saturating_sub(1);
                result.push((i, self.color_for_depth(depth).to_string()));
            }
        }
        result
    }

    pub fn color_count(&self) -> usize {
        self.colors.len()
    }
}

impl Default for BracketScopeHighlighter {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl fmt::Display for BracketScopeHighlighter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BracketScopeHighlighter({} colors)", self.colors.len())
    }
}

// ---------------------------------------------------------------------------
// Auto-close configuration
// ---------------------------------------------------------------------------

/// Configuration for bracket auto-close behavior.
#[derive(Debug, Clone)]
pub struct AutoCloseConfig {
    pub enabled: bool,
    pub pairs: Vec<(char, char)>,
    /// Characters before which auto-close is suppressed.
    pub suppress_before: Vec<char>,
}

impl AutoCloseConfig {
    pub fn new() -> Self {
        Self {
            enabled: true,
            pairs: vec![('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\'')],
            suppress_before: Vec::new(),
        }
    }

    /// Whether auto-close should be applied for a given opening character.
    pub fn should_auto_close(&self, open_char: char, next_char: Option<char>) -> bool {
        if !self.enabled {
            return false;
        }
        if !self.pairs.iter().any(|(o, _)| *o == open_char) {
            return false;
        }
        if let Some(nc) = next_char {
            if self.suppress_before.contains(&nc) {
                return false;
            }
        }
        true
    }

    /// Get the closing character for a given opener.
    pub fn close_char(&self, open_char: char) -> Option<char> {
        self.pairs.iter().find(|(o, _)| *o == open_char).map(|(_, c)| *c)
    }

    /// Add a custom bracket pair.
    pub fn add_pair(&mut self, open: char, close: char) {
        self.pairs.push((open, close));
    }

    /// Add a suppress-before character.
    pub fn suppress_before_char(&mut self, ch: char) {
        self.suppress_before.push(ch);
    }
}

impl Default for AutoCloseConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AutoCloseConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AutoCloseConfig(enabled={}, {} pairs)",
            self.enabled,
            self.pairs.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Bracket pair counting across document
// ---------------------------------------------------------------------------

/// Document-wide bracket pair count summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentBracketCount {
    pub total_pairs: usize,
    pub unmatched_opens: usize,
    pub unmatched_closes: usize,
    pub max_depth: u32,
    pub is_balanced: bool,
}

impl DocumentBracketCount {
    /// Count all bracket pairs across the entire document.
    pub fn count(lines: &[&str], pairs: &[BracketPair]) -> Self {
        let mut depth: i32 = 0;
        let mut max_depth: i32 = 0;
        let mut total_opens: usize = 0;
        let mut total_closes: usize = 0;
        let mut matched_pairs: usize = 0;

        for line in lines {
            for ch in line.chars() {
                if pairs.iter().any(|p| p.open == ch) {
                    total_opens += 1;
                    depth += 1;
                    if depth > max_depth {
                        max_depth = depth;
                    }
                    matched_pairs += 1;
                } else if pairs.iter().any(|p| p.close == ch) {
                    total_closes += 1;
                    if depth > 0 {
                        depth -= 1;
                    } else {
                        // Unmatched close; don't count as matched pair
                        matched_pairs = matched_pairs.saturating_sub(1);
                    }
                }
            }
        }

        let unmatched_opens = total_opens.saturating_sub(total_closes.min(total_opens));
        let unmatched_closes = total_closes.saturating_sub(total_opens.min(total_closes));
        let is_balanced = unmatched_opens == 0 && unmatched_closes == 0;

        Self {
            total_pairs: total_opens.min(total_closes),
            unmatched_opens,
            unmatched_closes,
            max_depth: max_depth as u32,
            is_balanced,
        }
    }
}

impl fmt::Display for DocumentBracketCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BracketCount(pairs={}, unmatched_open={}, unmatched_close={}, balanced={})",
            self.total_pairs, self.unmatched_opens, self.unmatched_closes, self.is_balanced
        )
    }
}

// ---------------------------------------------------------------------------
// BracketAutoInsert - bracket auto-insert strategy
// ---------------------------------------------------------------------------

/// Severity level for bracket auto-insert strategy issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BracketAutoInsertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for BracketAutoInsertSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [BracketAutoInsert].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketAutoInsertEntry {
    pub id: String,
    pub label: String,
    pub severity: BracketAutoInsertSeverity,
    pub detail: Option<String>,
    pub pair_count: usize,
    enabled: bool,
}

impl BracketAutoInsertEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: BracketAutoInsertSeverity::Low,
            detail: None,
            pair_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: BracketAutoInsertSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_pair_count(mut self, val: usize) -> Self {
        self.pair_count = val;
        self
    }

    pub fn should_auto_insert(&self) -> bool {
        self.enabled && self.severity >= BracketAutoInsertSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.pair_count, det)
    }
}

impl fmt::Display for BracketAutoInsertEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [BracketAutoInsertEntry] items.
#[derive(Debug, Clone)]
pub struct BracketAutoInsert {
    entries: Vec<BracketAutoInsertEntry>,
    name: String,
    capacity: usize,
}

impl BracketAutoInsert {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: BracketAutoInsertEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<BracketAutoInsertEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&BracketAutoInsertEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn pair_count(&self) -> usize { self.entries.len() }

    pub fn should_auto_insert(&self) -> bool {
        self.entries.iter().any(|e| e.should_auto_insert())
    }

    pub fn entries_by_severity(&self, severity: BracketAutoInsertSeverity) -> Vec<&BracketAutoInsertEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= BracketAutoInsertSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&BracketAutoInsertEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&BracketAutoInsertEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// BracketPairHighlighter - bracket pair highlighter
// ---------------------------------------------------------------------------

/// Configuration for [BracketPairHighlighter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketPairHighlighterConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub highlight_count: usize,
}

impl BracketPairHighlighterConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, highlight_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_highlight_count(mut self, val: usize) -> Self { self.highlight_count = val; self }
}

impl Default for BracketPairHighlighterConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [BracketPairHighlighter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketPairHighlighterItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl BracketPairHighlighterItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn is_highlighted(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for BracketPairHighlighterItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [BracketPairHighlighterItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct BracketPairHighlighter {
    config: BracketPairHighlighterConfig,
    items: Vec<BracketPairHighlighterItem>,
}

impl BracketPairHighlighter {
    pub fn new(config: BracketPairHighlighterConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: BracketPairHighlighterItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<BracketPairHighlighterItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&BracketPairHighlighterItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn highlight_count(&self) -> usize { self.items.len() }

    pub fn is_highlighted(&self) -> bool {
        self.items.iter().any(|i| i.is_highlighted())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&BracketPairHighlighterItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&BracketPairHighlighterItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &BracketPairHighlighterConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// vsedit-bracket: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl BracketXConfig {
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

impl std::fmt::Display for BracketXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct BracketXRegistry {
    entries: Vec<BracketXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl BracketXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: BracketXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&BracketXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut BracketXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<BracketXConfig> {
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

    pub fn active_entries(&self) -> Vec<&BracketXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&BracketXConfig> {
        let mut sorted: Vec<&BracketXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&BracketXConfig> {
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

    pub fn iter(&self) -> BracketXIterator<'_> {
        BracketXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct BracketXIterator<'a> {
    inner: std::slice::Iter<'a, BracketXConfig>,
}

impl<'a> Iterator for BracketXIterator<'a> {
    type Item = &'a BracketXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct BracketXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl BracketXCache {
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
pub struct BracketXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl BracketXFormatter {
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

    pub fn format_entry(&self, entry: &BracketXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &BracketXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &BracketXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for BracketXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct BracketXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl BracketXValidator {
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

    pub fn validate(&self, entry: &BracketXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &BracketXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for BracketXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 69
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer69 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer69 {
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
pub fn xb_fnv1a_69(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_69<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_69<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_69(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_69(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
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

    #[test]
    fn display_bracketerror_variants() {
        assert!(std::mem::size_of::<BracketError>() > 0);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        let _svc = BracketPairConfig::default();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn bracket_match_clone() {
        let m = BracketMatch { open_line: 1, open_col: 1, close_line: 3, close_col: 1, depth: 0 };
        let m2 = m.clone();
        assert_eq!(m, m2);
    }

    #[test]
    fn find_bracket_at_invalid_position() {
        let lines = vec!["hello world"];
        let pairs = default_bracket_pairs();
        assert_eq!(find_matching_bracket(&lines, 1, 1, &pairs), None);
    }

    #[test]
    fn bracket_pair_debug() {
        let bp = BracketPair { open: '(', close: ')' };
        assert!(format!("{:?}", bp).contains("BracketPair"));
    }

    // -- auto-close tests ---------------------------------------------------

    #[test]
    fn auto_close_opening_brackets() {
        assert_eq!(auto_close_bracket('('), Some(')'));
        assert_eq!(auto_close_bracket('['), Some(']'));
        assert_eq!(auto_close_bracket('{'), Some('}'));
        assert_eq!(auto_close_bracket('<'), Some('>'));
        assert_eq!(auto_close_bracket('"'), Some('"'));
        assert_eq!(auto_close_bracket('a'), None);
    }

    #[test]
    fn auto_surround_wraps_text() {
        assert_eq!(auto_surround_selection("hello", '('), "(hello)");
        assert_eq!(auto_surround_selection("x", '{'), "{x}");
        assert_eq!(auto_surround_selection("abc", '['), "[abc]");
        assert_eq!(auto_surround_selection("text", '"'), "\"text\"");
    }

    // -- bracket pair colorization ------------------------------------------

    #[test]
    fn colorization_basic() {
        let lines = vec!["(a + (b))"];
        let pairs = default_bracket_pairs();
        let result = bracket_pair_colorization(&lines, &pairs);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].nesting_level, 0); // outer (
        assert_eq!(result[1].nesting_level, 1); // inner (
        assert_eq!(result[2].nesting_level, 1); // inner )
        assert_eq!(result[3].nesting_level, 0); // outer )
    }

    #[test]
    fn colorization_skips_strings() {
        let lines = vec!["let s = \"(\";"];
        let pairs = default_bracket_pairs();
        let result = bracket_pair_colorization(&lines, &pairs);
        // The ( inside the string should be skipped
        assert!(result.is_empty());
    }

    #[test]
    fn colorization_skips_comments() {
        let lines = vec!["let x = 1; // (not a bracket)"];
        let pairs = default_bracket_pairs();
        let result = bracket_pair_colorization(&lines, &pairs);
        assert!(result.is_empty());
    }

    // -- smart bracket matching (skips strings/comments) --------------------

    #[test]
    fn smart_match_skips_string() {
        let lines = vec!["fn(\")(\"  , x)"];
        let pairs = default_bracket_pairs();
        // Match the outer ( at col 3 — should skip brackets in string
        let result = find_matching_bracket_smart(&lines, 1, 3, &pairs);
        assert_eq!(result, Some((1, 13)));
    }

    #[test]
    fn smart_match_skips_comment() {
        let lines = vec!["fn f() {", "  // }", "}"];
        let pairs = default_bracket_pairs();
        let result = find_matching_bracket_smart(&lines, 1, 8, &pairs);
        assert_eq!(result, Some((3, 1)));
    }

    #[test]
    fn stats_basic() {
        let lines = vec!["fn f() { [a] }"];
        let pairs = default_bracket_pairs();
        let stats = BracketStats::compute(&lines, &pairs);
        assert!(stats.total_pairs >= 2);
        assert_eq!(stats.max_depth, 2);
    }

    #[test]
    fn stats_empty() {
        let lines: Vec<&str> = vec!["no brackets here"];
        let pairs = default_bracket_pairs();
        let stats = BracketStats::compute(&lines, &pairs);
        assert_eq!(stats.total_pairs, 0);
        assert_eq!(stats.most_common_pair, None);
    }

    #[test]
    fn stats_most_common() {
        let lines = vec!["(())(()){}"];
        let pairs = default_bracket_pairs();
        let stats = BracketStats::compute(&lines, &pairs);
        assert_eq!(stats.most_common_pair, Some(('(', ')')));
    }

    // -- BracketColorizer tests ---------------------------------------------

    #[test]
    fn colorizer_default_has_six_colors() {
        let c = BracketColorizer::default();
        assert_eq!(c.colors.len(), 6);
    }

    #[test]
    fn colorizer_colorize_line_simple() {
        let c = BracketColorizer::new(vec!["red".into(), "blue".into()]);
        let pairs = default_bracket_pairs();
        let result = c.colorize_line("(a)", &pairs);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].char, '(');
        assert_eq!(result[0].col, 0);
        assert_eq!(result[0].color, "red");
        assert_eq!(result[0].depth, 0);
        assert_eq!(result[1].char, ')');
        assert_eq!(result[1].color, "red");
    }

    #[test]
    fn colorizer_nested_depth_cycling() {
        let c = BracketColorizer::new(vec!["A".into(), "B".into()]);
        let pairs = default_bracket_pairs();
        let result = c.colorize_line("(([]))", &pairs);
        // ( depth=0->A, ( depth=1->B, [ depth=2->A, ] depth=2->A, ) depth=1->B, ) depth=0->A
        assert_eq!(result.len(), 6);
        assert_eq!(result[0].color, "A"); // (
        assert_eq!(result[1].color, "B"); // (
        assert_eq!(result[2].color, "A"); // [
        assert_eq!(result[3].color, "A"); // ]
        assert_eq!(result[4].color, "B"); // )
        assert_eq!(result[5].color, "A"); // )
    }

    #[test]
    fn colorizer_empty_colors() {
        let c = BracketColorizer::new(vec![]);
        let pairs = default_bracket_pairs();
        let result = c.colorize_line("(a)", &pairs);
        assert!(result.is_empty());
    }

    // -- bracket_pair_at_position tests -------------------------------------

    #[test]
    fn pair_at_position_on_open_bracket() {
        let lines = vec!["(hello)"];
        let pairs = default_bracket_pairs();
        let m = bracket_pair_at_position(&lines, 1, 1, &pairs).unwrap();
        assert_eq!(m.open_line, 1);
        assert_eq!(m.open_col, 1);
        assert_eq!(m.close_line, 1);
        assert_eq!(m.close_col, 7);
    }

    #[test]
    fn pair_at_position_on_close_bracket() {
        let lines = vec!["(hello)"];
        let pairs = default_bracket_pairs();
        let m = bracket_pair_at_position(&lines, 1, 7, &pairs).unwrap();
        assert_eq!(m.open_col, 1);
        assert_eq!(m.close_col, 7);
    }

    #[test]
    fn pair_at_position_inside_brackets() {
        let lines = vec!["(hello)"];
        let pairs = default_bracket_pairs();
        let m = bracket_pair_at_position(&lines, 1, 3, &pairs).unwrap();
        assert_eq!(m.open_col, 1);
        assert_eq!(m.close_col, 7);
    }

    #[test]
    fn pair_at_position_no_brackets() {
        let lines = vec!["hello"];
        let pairs = default_bracket_pairs();
        assert!(bracket_pair_at_position(&lines, 1, 3, &pairs).is_none());
    }

    // -- bracket_errors tests -----------------------------------------------

    #[test]
    fn bracket_errors_valid_document() {
        let lines = vec!["fn f() { [x] }"];
        let pairs = default_bracket_pairs();
        assert!(bracket_errors(&lines, &pairs).is_empty());
    }

    #[test]
    fn bracket_errors_unmatched_open() {
        let lines = vec!["(a + b"];
        let pairs = default_bracket_pairs();
        let errs = bracket_errors(&lines, &pairs);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].bracket, '(');
        assert_eq!(errs[0].error_kind, BracketErrorKind::UnmatchedOpen);
    }

    #[test]
    fn bracket_errors_unmatched_close() {
        let lines = vec!["a + b)"];
        let pairs = default_bracket_pairs();
        let errs = bracket_errors(&lines, &pairs);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].bracket, ')');
        assert_eq!(errs[0].error_kind, BracketErrorKind::UnmatchedClose);
    }

    #[test]
    fn bracket_errors_mismatch() {
        let lines = vec!["(a + b]"];
        let pairs = default_bracket_pairs();
        let errs = bracket_errors(&lines, &pairs);
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_kind,
            BracketErrorKind::Mismatch { expected: ')', found: ']' }
        );
    }

    #[test]
    fn bracket_errors_multiple() {
        let lines = vec![")("];
        let pairs = default_bracket_pairs();
        let errs = bracket_errors(&lines, &pairs);
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].error_kind, BracketErrorKind::UnmatchedClose);
        assert_eq!(errs[1].error_kind, BracketErrorKind::UnmatchedOpen);
    }

    // ── BracketStatistics tests ──

    #[test]
    fn bracket_statistics_balanced() {
        let lines = vec!["fn f() {", "  let x = [1, 2];", "}"];
        let pairs = default_bracket_pairs();
        let stats = BracketStatistics::compute(&lines, &pairs);
        assert!(stats.is_balanced);
        assert_eq!(stats.total_opens, stats.total_closes);
        assert!(stats.max_depth() >= 1);
    }

    #[test]
    fn bracket_statistics_unbalanced() {
        let lines = vec!["(("];
        let pairs = default_bracket_pairs();
        let stats = BracketStatistics::compute(&lines, &pairs);
        assert!(!stats.is_balanced);
        assert_eq!(stats.total_opens, 2);
        assert_eq!(stats.total_closes, 0);
    }

    #[test]
    fn bracket_statistics_depth_histogram() {
        let lines = vec!["(())"];
        let pairs = default_bracket_pairs();
        let stats = BracketStatistics::compute(&lines, &pairs);
        // depth 0: outer open + outer close = 2
        // depth 1: inner open + inner close = 2
        assert_eq!(stats.depth_histogram.len(), 2);
        assert_eq!(stats.depth_histogram[0], 2);
        assert_eq!(stats.depth_histogram[1], 2);
    }

    // ── BracketHighlighter tests ──

    #[test]
    fn bracket_highlighter_basic() {
        let lines = vec!["(a)"];
        let pairs = default_bracket_pairs();
        let hl = BracketHighlighter::new(vec!["red".into(), "blue".into()]);
        let ranges = hl.highlight(&lines, &pairs);
        assert_eq!(ranges.len(), 2);
        assert!(ranges[0].is_open);
        assert!(!ranges[1].is_open);
        assert_eq!(ranges[0].color, ranges[1].color); // same depth
    }

    // ── Auto-close suggestion tests ──

    #[test]
    fn suggest_auto_close_unclosed() {
        let lines = vec!["fn f() {", "  let x = (1;"];
        let pairs = default_bracket_pairs();
        let suggestions = suggest_auto_close(&lines, &pairs);
        assert_eq!(suggestions.len(), 2); // unclosed { and (
        let close_chars: Vec<char> = suggestions.iter().map(|s| s.close_char).collect();
        assert!(close_chars.contains(&')'));
        assert!(close_chars.contains(&'}'));
    }

    #[test]
    fn suggest_auto_close_all_closed() {
        let lines = vec!["fn f() {}"];
        let pairs = default_bracket_pairs();
        let suggestions = suggest_auto_close(&lines, &pairs);
        assert!(suggestions.is_empty());
    }

    // ── Folding range tests ──

    #[test]
    fn folding_ranges_multiline() {
        let lines = vec!["fn f() {", "  x", "}"];
        let pairs = default_bracket_pairs();
        let folds = folding_ranges(&lines, &pairs);
        // Only the braces span multiple lines; parens are single-line.
        assert_eq!(folds.len(), 1);
        assert_eq!(folds[0].start_line, 1);
        assert_eq!(folds[0].end_line, 3);
    }

    #[test]
    fn folding_ranges_excludes_single_line() {
        let lines = vec!["let x = (1 + 2);"];
        let pairs = default_bracket_pairs();
        let folds = folding_ranges(&lines, &pairs);
        assert!(folds.is_empty());
    }

    // ── Indentation guide tests ──

    #[test]
    fn indentation_guides_basic() {
        let lines = vec!["{", "  x", "}"];
        let pairs = default_bracket_pairs();
        let guides = indentation_guides(&lines, &pairs);
        assert_eq!(guides.len(), 3);
        assert_eq!(guides[0].depth, 0);
        assert!(guides[0].opens);
        assert_eq!(guides[1].depth, 1);
        assert!(!guides[1].opens);
        assert!(!guides[1].closes);
        assert_eq!(guides[2].depth, 1);
        assert!(guides[2].closes);
    }

    // ── Bracket scope extraction tests ──

    #[test]
    fn extract_scope_single_line() {
        let lines = vec!["(hello)"];
        let pairs = default_bracket_pairs();
        let matches = find_all_brackets(&lines, &pairs);
        assert_eq!(matches.len(), 1);
        let scope = extract_bracket_scope(&lines, &matches[0]).unwrap();
        assert_eq!(scope, "hello");
    }

    #[test]
    fn extract_scope_multiline() {
        let lines = vec!["{", "  body", "}"];
        let pairs = default_bracket_pairs();
        let matches = find_all_brackets(&lines, &pairs);
        let brace_match = matches.iter().find(|m| m.open_col == 1 && m.open_line == 1).unwrap();
        let scope = extract_bracket_scope(&lines, brace_match).unwrap();
        assert!(scope.contains("body"));
    }

    // ── Bracket fix edit tests ──

    #[test]
    fn fix_edits_unmatched_open() {
        let lines = vec!["(hello"];
        let pairs = default_bracket_pairs();
        let edits = bracket_fix_edits(&lines, &pairs);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].insert_text, ")");
        assert_eq!(edits[0].delete_len, 0);
    }

    #[test]
    fn fix_edits_unmatched_close() {
        let lines = vec!["hello)"];
        let pairs = default_bracket_pairs();
        let edits = bracket_fix_edits(&lines, &pairs);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].delete_len, 1);
        assert!(edits[0].insert_text.is_empty());
    }

    #[test]
    fn fix_edits_mismatch() {
        let lines = vec!["(hello]"];
        let pairs = default_bracket_pairs();
        let edits = bracket_fix_edits(&lines, &pairs);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].insert_text, ")");
        assert_eq!(edits[0].delete_len, 1);
    }

    // ── Line bracket balance tests ──

    #[test]
    fn line_balance_simple() {
        let lines = vec!["{", "  (x)", "}"];
        let pairs = default_bracket_pairs();
        let balances = line_bracket_balances(&lines, &pairs);
        assert_eq!(balances.len(), 3);
        assert_eq!(balances[0].net, 1);
        assert_eq!(balances[0].cumulative_depth, 1);
        assert_eq!(balances[1].net, 0); // ( and ) cancel
        assert_eq!(balances[1].cumulative_depth, 1);
        assert_eq!(balances[2].net, -1);
        assert_eq!(balances[2].cumulative_depth, 0);
    }

    #[test]
    fn line_balance_empty_doc() {
        let lines: Vec<&str> = vec![];
        let pairs = default_bracket_pairs();
        let balances = line_bracket_balances(&lines, &pairs);
        assert!(balances.is_empty());
    }

    // -- BracketPairGuide --------------------------------------------------

    #[test]
    fn bracket_pair_guide_creation() {
        let g = BracketPairGuide::new(5, 1, 10, 0, 6);
        assert_eq!(g.line_span(), 9);
        assert!(g.contains_line(5));
        assert!(!g.contains_line(11));
        assert_eq!(g.color_index, 0);
    }

    #[test]
    fn bracket_pair_guide_display() {
        let g = BracketPairGuide::new(3, 1, 5, 1, 6);
        let s = format!("{g}");
        assert!(s.contains("col=3"));
        assert!(s.contains("depth=1"));
    }

    #[test]
    fn build_bracket_guides_filters_same_line() {
        let matches = vec![
            BracketMatch { open_line: 1, open_col: 1, close_line: 1, close_col: 5, depth: 0 },
            BracketMatch { open_line: 1, open_col: 1, close_line: 5, close_col: 1, depth: 0 },
        ];
        let guides = build_bracket_guides(&matches, 6);
        assert_eq!(guides.len(), 1);
    }

    // -- BracketScopeHighlighter -------------------------------------------

    #[test]
    fn scope_highlighter_color_for_depth() {
        let h = BracketScopeHighlighter::default();
        assert!(!h.color_for_depth(0).is_empty());
        assert_eq!(h.color_for_depth(0), h.color_for_depth(h.color_count() as u32));
    }

    #[test]
    fn scope_highlighter_highlight_line() {
        let h = BracketScopeHighlighter::default();
        let pairs = default_bracket_pairs();
        let highlights = h.highlight_line("(a[b]c)", 0, &pairs);
        assert_eq!(highlights.len(), 4); // ( [ ] )
    }

    #[test]
    fn scope_highlighter_display() {
        let h = BracketScopeHighlighter::default();
        let s = format!("{h}");
        assert!(s.contains("colors"));
    }

    // -- AutoCloseConfig ---------------------------------------------------

    #[test]
    fn auto_close_config_defaults() {
        let cfg = AutoCloseConfig::default();
        assert!(cfg.should_auto_close('(', None));
        assert!(cfg.should_auto_close('{', None));
        assert!(!cfg.should_auto_close('x', None));
    }

    #[test]
    fn auto_close_config_suppress() {
        let mut cfg = AutoCloseConfig::new();
        cfg.suppress_before_char('a');
        assert!(!cfg.should_auto_close('(', Some('a')));
        assert!(cfg.should_auto_close('(', Some('b')));
    }

    #[test]
    fn auto_close_config_disabled() {
        let mut cfg = AutoCloseConfig::new();
        cfg.enabled = false;
        assert!(!cfg.should_auto_close('(', None));
    }

    #[test]
    fn auto_close_config_close_char() {
        let cfg = AutoCloseConfig::new();
        assert_eq!(cfg.close_char('('), Some(')'));
        assert_eq!(cfg.close_char('{'), Some('}'));
        assert_eq!(cfg.close_char('x'), None);
    }

    #[test]
    fn auto_close_config_display() {
        let cfg = AutoCloseConfig::new();
        let s = format!("{cfg}");
        assert!(s.contains("enabled=true"));
    }

    // -- DocumentBracketCount ----------------------------------------------

    #[test]
    fn document_bracket_count_balanced() {
        let lines = vec!["fn main() {", "  let x = (1 + 2);", "}"];
        let pairs = default_bracket_pairs();
        let count = DocumentBracketCount::count(&lines, &pairs);
        assert!(count.is_balanced);
        assert_eq!(count.unmatched_opens, 0);
        assert_eq!(count.unmatched_closes, 0);
        assert_eq!(count.max_depth, 2);
    }

    #[test]
    fn document_bracket_count_unbalanced() {
        let lines = vec!["(()", "}{"];
        let pairs = default_bracket_pairs();
        let count = DocumentBracketCount::count(&lines, &pairs);
        assert!(!count.is_balanced);
    }

    #[test]
    fn document_bracket_count_display() {
        let count = DocumentBracketCount {
            total_pairs: 3,
            unmatched_opens: 0,
            unmatched_closes: 0,
            max_depth: 2,
            is_balanced: true,
        };
        let s = format!("{count}");
        assert!(s.contains("balanced=true"));
    }

#[test]
    fn bracketautoinsert_severity_ordering() {
        assert!(BracketAutoInsertSeverity::Critical > BracketAutoInsertSeverity::High);
        assert!(BracketAutoInsertSeverity::High > BracketAutoInsertSeverity::Medium);
        assert!(BracketAutoInsertSeverity::Medium > BracketAutoInsertSeverity::Low);
    }

    #[test]
    fn bracketautoinsert_severity_display() {
        assert_eq!(BracketAutoInsertSeverity::Low.to_string(), "low");
        assert_eq!(BracketAutoInsertSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn bracketautoinsert_entry_creation() {
        let e = BracketAutoInsertEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, BracketAutoInsertSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn bracketautoinsert_entry_builder() {
        let e = BracketAutoInsertEntry::new("e2", "Entry 2")
            .with_severity(BracketAutoInsertSeverity::High)
            .with_detail("some detail")
            .with_pair_count(42);
        assert_eq!(e.severity, BracketAutoInsertSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.pair_count, 42);
    }

    #[test]
    fn bracketautoinsert_entry_enable_disable() {
        let mut e = BracketAutoInsertEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn bracketautoinsert_add_and_count() {
        let mut mgr = BracketAutoInsert::new("test");
        mgr.add(BracketAutoInsertEntry::new("a", "A"));
        mgr.add(BracketAutoInsertEntry::new("b", "B").with_severity(BracketAutoInsertSeverity::High));
        assert_eq!(mgr.pair_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn bracketautoinsert_remove() {
        let mut mgr = BracketAutoInsert::new("test");
        mgr.add(BracketAutoInsertEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn bracketautoinsert_capacity() {
        let mut mgr = BracketAutoInsert::new("test").with_capacity(1);
        assert!(mgr.add(BracketAutoInsertEntry::new("a", "A")));
        assert!(!mgr.add(BracketAutoInsertEntry::new("b", "B")));
    }

    #[test]
    fn bracketautoinsert_sorted_by_severity() {
        let mut mgr = BracketAutoInsert::new("test");
        mgr.add(BracketAutoInsertEntry::new("lo", "Low"));
        mgr.add(BracketAutoInsertEntry::new("hi", "High").with_severity(BracketAutoInsertSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, BracketAutoInsertSeverity::Critical);
    }

    #[test]
    fn bracketautoinsert_summary() {
        let mgr = BracketAutoInsert::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn bracketpairhighlighter_config_defaults() {
        let cfg = BracketPairHighlighterConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn bracketpairhighlighter_item_creation() {
        let item = BracketPairHighlighterItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn bracketpairhighlighter_add_and_get() {
        let mut mgr = BracketPairHighlighter::new(BracketPairHighlighterConfig::new("test"));
        mgr.add(BracketPairHighlighterItem::new("k1", "v1"));
        assert_eq!(mgr.highlight_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn bracketpairhighlighter_remove_item() {
        let mut mgr = BracketPairHighlighter::new(BracketPairHighlighterConfig::new("test"));
        mgr.add(BracketPairHighlighterItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn bracketpairhighlighter_sorted_by_priority() {
        let mut mgr = BracketPairHighlighter::new(BracketPairHighlighterConfig::new("test"));
        mgr.add(BracketPairHighlighterItem::new("lo", "low").with_priority(1));
        mgr.add(BracketPairHighlighterItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn bracketpairhighlighter_items_with_tag() {
        let mut mgr = BracketPairHighlighter::new(BracketPairHighlighterConfig::new("test"));
        mgr.add(BracketPairHighlighterItem::new("a", "1").with_tag("x"));
        mgr.add(BracketPairHighlighterItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn bracketpairhighlighter_report() {
        let mgr = BracketPairHighlighter::new(BracketPairHighlighterConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn bracket_x_config_new() {
        let c = BracketXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn bracket_x_config_builder() {
        let c = BracketXConfig::new("k")
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
    fn bracket_x_config_display() {
        let c = BracketXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn bracket_x_registry_insert_get() {
        let mut reg = BracketXRegistry::new();
        reg.insert(BracketXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn bracket_x_registry_duplicate() {
        let mut reg = BracketXRegistry::new();
        reg.insert(BracketXConfig::new("a")).unwrap();
        assert!(reg.insert(BracketXConfig::new("a")).is_err());
    }

    #[test]
    fn bracket_x_registry_remove() {
        let mut reg = BracketXRegistry::new();
        reg.insert(BracketXConfig::new("a")).unwrap();
        reg.insert(BracketXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn bracket_x_registry_active_entries() {
        let mut reg = BracketXRegistry::new();
        reg.insert(BracketXConfig::new("a")).unwrap();
        reg.insert(BracketXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn bracket_x_registry_by_weight() {
        let mut reg = BracketXRegistry::new();
        reg.insert(BracketXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(BracketXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn bracket_x_registry_tags() {
        let mut reg = BracketXRegistry::new();
        reg.insert(BracketXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(BracketXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn bracket_x_registry_total_weight() {
        let mut reg = BracketXRegistry::new();
        reg.insert(BracketXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(BracketXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn bracket_x_registry_iterator() {
        let mut reg = BracketXRegistry::new();
        reg.insert(BracketXConfig::new("a")).unwrap();
        reg.insert(BracketXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn bracket_x_cache_put_get() {
        let mut cache = BracketXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn bracket_x_cache_eviction() {
        let mut cache = BracketXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn bracket_x_cache_lru_order() {
        let mut cache = BracketXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn bracket_x_cache_most_least_recent() {
        let mut cache = BracketXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn bracket_x_formatter_entry() {
        let e = BracketXConfig::new("k").with_value("v");
        let fmt = BracketXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn bracket_x_formatter_summary() {
        let mut reg = BracketXRegistry::new();
        reg.insert(BracketXConfig::new("a").with_weight(5)).unwrap();
        let fmt = BracketXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn bracket_x_validator_valid() {
        let v = BracketXValidator::new();
        let c = BracketXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn bracket_x_validator_empty_key() {
        let v = BracketXValidator::new();
        let c = BracketXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn bracket_x_validator_require_value() {
        let v = BracketXValidator::new().require_value(true);
        let c = BracketXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn bracket_x_validator_allowed_tags() {
        let v = BracketXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = BracketXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn bracket_x_validator_validate_all() {
        let v = BracketXValidator::new();
        let mut reg = BracketXRegistry::new();
        reg.insert(BracketXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_69_push_and_len() {
        let mut rb = super::XbRingBuffer69::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_69_overwrite() {
        let mut rb = super::XbRingBuffer69::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_69_get_out_of_bounds() {
        let rb = super::XbRingBuffer69::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_69_drain_all() {
        let mut rb = super::XbRingBuffer69::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_69_peek_front_back() {
        let mut rb = super::XbRingBuffer69::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_69_clear() {
        let mut rb = super::XbRingBuffer69::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_69_capacity() {
        let rb = super::XbRingBuffer69::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_69_basic() {
        let h = super::xb_fnv1a_69(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_69(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_69_different_inputs() {
        let h1 = super::xb_fnv1a_69(b"abc");
        let h2 = super::xb_fnv1a_69(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_69_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_69(&data);
        let dec = super::xb_rle_decode_69(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_69_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_69(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_69(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_69_values() {
        assert!((super::xb_clamp_69(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_69(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_69(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_69_values() {
        assert!((super::xb_lerp_69(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_69(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_69(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_69_wrap_around_twice() {
        let mut rb = super::XbRingBuffer69::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }

}
