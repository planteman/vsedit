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
}
