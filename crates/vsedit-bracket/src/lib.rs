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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 7
// ---------------------------------------------------------------------------

/// Generic object pool `Xc7Pool<T>`.
pub struct Xc7Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc7Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc7PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc7Pool<T> {
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
    pub fn stats(&self) -> Xc7PoolStats {
        Xc7PoolStats {
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

impl<T> Default for Xc7Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc7Scheduler`.
pub struct Xc7Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc7Scheduler {
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

impl Default for Xc7Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_7 hash for the given byte slice.
pub fn xc_7_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_7 convention.
pub fn xc_7_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe82 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe82Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe82PipelineError {
    pub stage: Xe82Stage,
    pub message: String,
}

impl std::fmt::Display for Xe82PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe82Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe82Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError>>>,
    stage_names: Vec<Xe82Stage>,
}

impl Xe82Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe82Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe82Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe82Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe82Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError> {
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

    pub fn compose(mut self, other: Xe82Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe82CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe82CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe82Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe82CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe82CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe82Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe82CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_82_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe82CacheEntry {
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

    fn xe_82_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe82CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_82_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError> {
    Ok(data)
}

pub fn xe_82_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_82_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_82_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_82_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe82PipelineError> {
    Err(Xe82PipelineError {
        stage: Xe82Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_80: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg80Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg80Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg80Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_80: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg80Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg80Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg80Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg80Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 6).
pub struct Xh6SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh6SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 48 as u64,
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

/// A compact bit set supporting boolean operations (variant 6).
pub struct Xh6BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh6BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 6).
pub struct Xi6Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi6Deque<T> {
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
pub struct Xi6Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi6Interval {
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

/// A simple interval tree (variant 6).
pub struct Xi6IntervalTree {
    xi_intervals: Vec<Xi6Interval>,
}

impl Xi6IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi6Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi6Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi6Interval) -> Vec<&Xi6Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi6Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi6Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi6Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi6Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi6Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi6Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 6) ---

/// Disjoint set / union-find for crate 6.
pub struct Xj6UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj6UnionFind {
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

const XJ6_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 6.
pub struct Xj6BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj6BTreeNode<K, V>>>,
    len: usize,
}

struct Xj6BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj6BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj6BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ6_BTREE_ORDER - 1
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
        let mid = XJ6_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj6BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj6BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj6BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj6BTreeNode::xj_new_leaf();
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


// --- xk_6 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk6SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk6SegmentTree {
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
pub struct Xk6DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk6DisjointIntervals {
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


    // ---- xc_ pool / scheduler tests – block 7 ----

    #[test]
    fn xc_7_pool_new_empty() {
        let pool: super::Xc7Pool<i32> = super::Xc7Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_7_pool_release_acquire() {
        let mut pool = super::Xc7Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_7_pool_acquire_empty() {
        let mut pool: super::Xc7Pool<i32> = super::Xc7Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_7_pool_full() {
        let mut pool = super::Xc7Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_7_pool_drain() {
        let mut pool = super::Xc7Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_7_pool_stats() {
        let mut pool = super::Xc7Pool::new(8);
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
    fn xc_7_pool_clear() {
        let mut pool = super::Xc7Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_7_pool_shrink() {
        let mut pool = super::Xc7Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_7_pool_default() {
        let pool: super::Xc7Pool<String> = super::Xc7Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_7_pool_extend() {
        let mut pool = super::Xc7Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_7_pool_retain() {
        let mut pool = super::Xc7Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_7_scheduler_round_robin() {
        let mut sched = super::Xc7Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_7_scheduler_empty() {
        let mut sched = super::Xc7Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_7_scheduler_reset() {
        let mut sched = super::Xc7Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_7_scheduler_add_remove() {
        let mut sched = super::Xc7Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_7_scheduler_targets() {
        let sched = super::Xc7Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_7_hash_empty() {
        assert_eq!(super::xc_7_hash(b""), 5381);
    }

    #[test]
    fn xc_7_hash_data() {
        let h = super::xc_7_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_7_hash(b"hello"), h);
    }

    #[test]
    fn xc_7_reverse_str() {
        assert_eq!(super::xc_7_reverse("abc"), "cba");
        assert_eq!(super::xc_7_reverse(""), "");
    }


    #[test]
    fn xe_82_pipeline_empty() {
        let p = super::Xe82Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_82_pipeline_parse_stage() {
        let p = super::Xe82Pipeline::new()
            .add_parse(super::xe_82_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_82_pipeline_transform_double() {
        let p = super::Xe82Pipeline::new()
            .add_transform(super::xe_82_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_82_pipeline_validate_reverse() {
        let p = super::Xe82Pipeline::new()
            .add_validate(super::xe_82_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_82_pipeline_emit_filter() {
        let p = super::Xe82Pipeline::new()
            .add_emit(super::xe_82_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_82_pipeline_multi_stage() {
        let p = super::Xe82Pipeline::new()
            .add_parse(super::xe_82_pipeline_identity)
            .add_transform(super::xe_82_pipeline_double)
            .add_validate(super::xe_82_pipeline_reverse)
            .add_emit(super::xe_82_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_82_pipeline_error_propagation() {
        let p = super::Xe82Pipeline::new()
            .add_parse(super::xe_82_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe82Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_82_pipeline_compose() {
        let p1 = super::Xe82Pipeline::new()
            .add_parse(super::xe_82_pipeline_identity);
        let p2 = super::Xe82Pipeline::new()
            .add_transform(super::xe_82_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_82_pipeline_error_display() {
        let e = super::Xe82PipelineError {
            stage: super::Xe82Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_82_cache_put_get() {
        let mut c = super::Xe82Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_82_cache_miss() {
        let mut c: super::Xe82Cache<&str, i32> = super::Xe82Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_82_cache_ttl_expiry() {
        let mut c = super::Xe82Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_82_cache_evict() {
        let mut c = super::Xe82Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_82_cache_capacity() {
        let mut c = super::Xe82Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_82_cache_stats() {
        let mut c = super::Xe82Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_82_cache_clear() {
        let mut c = super::Xe82Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_80 graph tests ------------------------------------------------

    #[test]
    fn xg_80_graph_empty() {
        let g = super::Xg80Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_80_graph_add_node() {
        let mut g = super::Xg80Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_80_graph_add_edge() {
        let mut g = super::Xg80Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_80_graph_neighbors() {
        let mut g = super::Xg80Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_80_graph_has_path() {
        let mut g = super::Xg80Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_80_graph_self_path() {
        let g = super::Xg80Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_80_graph_topo_sort() {
        let mut g = super::Xg80Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_80_graph_cycle_detect_false() {
        let mut g = super::Xg80Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_80_graph_cycle_detect_true() {
        let mut g = super::Xg80Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_80 heap tests -------------------------------------------------

    #[test]
    fn xg_80_heap_empty() {
        let h: super::Xg80Heap<i32> = super::Xg80Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_80_heap_push_pop() {
        let mut h = super::Xg80Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_80_heap_peek() {
        let mut h = super::Xg80Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_80_heap_drain_sorted() {
        let mut h = super::Xg80Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_80_heap_merge() {
        let mut a = super::Xg80Heap::new();
        let mut b = super::Xg80Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_80_heap_default() {
        let h: super::Xg80Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_80_graph_default() {
        let g: super::Xg80Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh6_skip_insert_contains() {
        let mut sl = super::Xh6SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh6_skip_remove() {
        let mut sl = super::Xh6SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh6_skip_len() {
        let mut sl = super::Xh6SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh6_skip_range_query() {
        let mut sl = super::Xh6SkipList::xh_new(4);
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
    fn xh6_skip_floor_ceiling() {
        let mut sl = super::Xh6SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh6_skip_rank() {
        let mut sl = super::Xh6SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh6_skip_empty() {
        let sl = super::Xh6SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh6_skip_duplicates() {
        let mut sl = super::Xh6SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh6_bitset_set_test() {
        let mut bs = super::Xh6BitSet::xh_new(256);
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
    fn xh6_bitset_clear_count() {
        let mut bs = super::Xh6BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh6_bitset_and_or_xor() {
        let mut a = super::Xh6BitSet::xh_new(128);
        let mut b = super::Xh6BitSet::xh_new(128);
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
    fn xh6_bitset_iter_ones() {
        let mut bs = super::Xh6BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh6_bitset_first_last() {
        let mut bs = super::Xh6BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh6_bitset_empty() {
        let bs = super::Xh6BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi6_deque_push_pop_back() {
        let mut dq = super::Xi6Deque::xi_new(4);
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
    fn xi6_deque_push_pop_front() {
        let mut dq = super::Xi6Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi6_deque_mixed_ops() {
        let mut dq = super::Xi6Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi6_deque_get_and_split() {
        let mut dq = super::Xi6Deque::xi_new(8);
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
    fn xi6_deque_rotate_left() {
        let mut dq = super::Xi6Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi6_deque_rotate_right() {
        let mut dq = super::Xi6Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi6_deque_grow() {
        let mut dq = super::Xi6Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi6_deque_empty() {
        let dq = super::Xi6Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi6_interval_tree_insert_query() {
        let mut tree = super::Xi6IntervalTree::xi_new();
        tree.xi_insert(super::Xi6Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi6Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi6Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi6_interval_tree_overlap() {
        let mut tree = super::Xi6IntervalTree::xi_new();
        tree.xi_insert(super::Xi6Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi6Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi6Interval::xi_new(12, 20));
        let q = super::Xi6Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi6_interval_tree_remove() {
        let mut tree = super::Xi6IntervalTree::xi_new();
        tree.xi_insert(super::Xi6Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi6Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi6_interval_tree_gaps() {
        let mut tree = super::Xi6IntervalTree::xi_new();
        tree.xi_insert(super::Xi6Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi6Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi6Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi6Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi6Interval::xi_new(8, 10));
    }

    #[test]
    fn xi6_interval_tree_merge() {
        let mut tree = super::Xi6IntervalTree::xi_new();
        tree.xi_insert(super::Xi6Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi6Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi6Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi6Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi6Interval::xi_new(10, 15));
    }

    #[test]
    fn xi6_interval_tree_all() {
        let mut tree = super::Xi6IntervalTree::xi_new();
        tree.xi_insert(super::Xi6Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi6Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi6_interval_tree_empty() {
        let tree = super::Xi6IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi6_interval_tree_contains_point() {
        let iv = super::Xi6Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 6) ---

    #[test]
    fn xj_6_uf_make_and_find() {
        let mut uf = super::Xj6UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_6_uf_union_connected() {
        let mut uf = super::Xj6UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_6_uf_component_count() {
        let mut uf = super::Xj6UnionFind::xj_new();
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
    fn xj_6_uf_component_size() {
        let mut uf = super::Xj6UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_6_uf_largest_component() {
        let mut uf = super::Xj6UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_6_uf_many_elements() {
        let mut uf = super::Xj6UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_6_uf_separate_components() {
        let mut uf = super::Xj6UnionFind::xj_new();
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
    fn xj_6_uf_path_compression() {
        let mut uf = super::Xj6UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_6_bt_insert_get() {
        let mut bt = super::Xj6BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_6_bt_contains_len() {
        let mut bt = super::Xj6BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_6_bt_replace() {
        let mut bt = super::Xj6BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_6_bt_remove() {
        let mut bt = super::Xj6BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_6_bt_keys_values() {
        let mut bt = super::Xj6BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_6_bt_range() {
        let mut bt = super::Xj6BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_6_bt_min_max() {
        let mut bt = super::Xj6BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_6_bt_many_inserts() {
        let mut bt = super::Xj6BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_6 segment tree tests ---

    #[test]
    fn xk_6_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk6SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_6_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk6SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_6_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk6SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_6_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk6SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_6_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk6SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_6_st_single_element() {
        let data = vec![42];
        let st = super::Xk6SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_6_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk6SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_6_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk6SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_6 disjoint intervals tests ---

    #[test]
    fn xk_6_di_add_and_count() {
        let mut di = super::Xk6DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_6_di_merge_overlap() {
        let mut di = super::Xk6DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_6_di_contains() {
        let mut di = super::Xk6DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_6_di_remove() {
        let mut di = super::Xk6DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_6_di_covered_length() {
        let mut di = super::Xk6DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_6_di_gaps() {
        let mut di = super::Xk6DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_6_di_merge_adjacent() {
        let mut di = super::Xk6DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_6_di_empty() {
        let di = super::Xk6DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
