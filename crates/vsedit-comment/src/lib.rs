//! VS Code–style line and block comment toggling.
//!
//! Provides enums and helpers to add, remove, or toggle line and block
//! comments for any language whose comment syntax is described by a
//! [`CommentRule`].

use std::fmt;
/// Whether a comment operation targets lines or blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentMode {
    /// Single-line comments (e.g. `//`).
    Line,
    /// Block comments (e.g. `/* … */`).
    Block,
}

/// Language-specific comment syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRule {
    /// Line-comment prefix, e.g. `"//"`.
    pub line_comment: Option<String>,
    /// Block-comment open/close pair, e.g. `("/*", "*/")`.
    pub block_comment: Option<(String, String)>,
}

impl CommentRule {
    /// Returns `true` if this rule has block-comment open/close markers.
    pub fn supports_block(&self) -> bool {
        self.block_comment.is_some()
    }

    /// Returns `true` if this rule has a line-comment prefix.
    pub fn supports_line(&self) -> bool {
        self.line_comment.is_some()
    }

    /// Return the comment rule for a well-known language identifier.
    ///
    /// Returns `None` for unrecognised languages.
    pub fn for_language(lang: &str) -> Option<Self> {
        match lang {
            "rust" => Some(Self {
                line_comment: Some("//".into()),
                block_comment: Some(("/*".into(), "*/".into())),
            }),
            "javascript" | "typescript" | "java" | "c" | "cpp" => Some(Self {
                line_comment: Some("//".into()),
                block_comment: Some(("/*".into(), "*/".into())),
            }),
            "go" => Some(Self {
                line_comment: Some("//".into()),
                block_comment: Some(("/*".into(), "*/".into())),
            }),
            "python" | "ruby" | "shell" => Some(Self {
                line_comment: Some("#".into()),
                block_comment: None,
            }),
            "html" | "xml" => Some(Self {
                line_comment: None,
                block_comment: Some(("<!--".into(), "-->".into())),
            }),
            "css" => Some(Self {
                line_comment: None,
                block_comment: Some(("/*".into(), "*/".into())),
            }),
            _ => None,
        }
    }
}

/// The action to perform when toggling a comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentAction {
    /// Add a comment if absent, remove it if present.
    Toggle,
    /// Unconditionally add a comment.
    Add,
    /// Unconditionally remove a comment.
    Remove,
}

/// Errors that can occur during comment operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentError {
    /// The language has no comment syntax for the requested mode.
    NoCommentSyntax,
    /// The supplied range is out of bounds or otherwise invalid.
    InvalidRange,
    /// The selection is empty; nothing to comment.
    EmptySelection,
}

impl std::fmt::Display for CommentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCommentSyntax => write!(f, "no comment syntax available for this language"),
            Self::InvalidRange => write!(f, "invalid range"),
            Self::EmptySelection => write!(f, "empty selection"),
        }
    }
}

impl std::fmt::Display for CommentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Line => write!(f, "line"),
            Self::Block => write!(f, "block"),
        }
    }
}

impl CommentMode {
    /// Returns a human-readable label for this mode.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Block => "block",
        }
    }
}

impl std::fmt::Display for CommentAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Toggle => write!(f, "toggle"),
            Self::Add => write!(f, "add"),
            Self::Remove => write!(f, "remove"),
        }
    }
}

/// Returns `true` when `line` (after stripping leading whitespace) starts
/// with `prefix`.
pub fn is_line_commented(line: &str, prefix: &str) -> bool {
    line.trim_start().starts_with(prefix)
}

/// Toggle (add or remove) a line-comment `prefix` on every line in `lines`.
///
/// * If **all** non-empty lines already start with `prefix` (after
///   indentation), the prefix is removed.
/// * Otherwise the prefix is prepended (after existing indentation) to every
///   line.
///
/// Empty lines are left untouched.
pub fn toggle_line_comment(lines: &[&str], prefix: &str) -> Vec<String> {
    let all_commented = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .all(|l| is_line_commented(l, prefix));

    if all_commented {
        lines.iter().map(|l| remove_line_prefix(l, prefix)).collect()
    } else {
        lines.iter().map(|l| add_line_prefix(l, prefix)).collect()
    }
}

/// Toggle a block comment around `text`.
///
/// * If `text` is already wrapped in `open` … `close` (after trimming),
///   the markers are removed.
/// * Otherwise the markers are added.
pub fn toggle_block_comment(text: &str, open: &str, close: &str) -> String {
    let trimmed = text.trim();
    if trimmed.starts_with(open) && trimmed.ends_with(close) {
        remove_block_comment(text, open, close)
    } else {
        format!("{open} {text} {close}")
    }
}

// ── internal helpers ───────────────────────────────────────────────────

fn add_line_prefix(line: &str, prefix: &str) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);
    format!("{indent}{prefix} {rest}")
}

fn remove_line_prefix(line: &str, prefix: &str) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);
    let stripped = rest
        .strip_prefix(prefix)
        .unwrap_or(rest);
    // Also strip a single trailing space after the prefix.
    let stripped = stripped.strip_prefix(' ').unwrap_or(stripped);
    format!("{indent}{stripped}")
}

fn remove_block_comment(text: &str, open: &str, close: &str) -> String {
    let trimmed = text.trim();
    let inner = &trimmed[open.len()..trimmed.len() - close.len()];
    // Strip one optional space on each side of the inner text.
    let inner = inner.strip_prefix(' ').unwrap_or(inner);
    let inner = inner.strip_suffix(' ').unwrap_or(inner);
    inner.to_string()
}

// ── higher-level operations ───────────────────────────────────────────

/// Apply a [`CommentAction`] to every line using a line-comment prefix.
///
/// Returns `Err` if `prefix` is empty or `lines` is empty.
pub fn apply_comment_action(
    lines: &[&str],
    prefix: &str,
    action: CommentAction,
) -> Result<Vec<String>, CommentError> {
    if lines.is_empty() {
        return Err(CommentError::EmptySelection);
    }
    if prefix.is_empty() {
        return Err(CommentError::NoCommentSyntax);
    }
    match action {
        CommentAction::Toggle => Ok(toggle_line_comment(lines, prefix)),
        CommentAction::Add => Ok(lines.iter().map(|l| add_line_prefix(l, prefix)).collect()),
        CommentAction::Remove => {
            Ok(lines.iter().map(|l| remove_line_prefix(l, prefix)).collect())
        }
    }
}

/// Apply a [`CommentAction`] to `text` using block-comment markers.
///
/// Returns `Err` if `open`/`close` are empty or `text` is empty.
pub fn apply_block_comment_action(
    text: &str,
    open: &str,
    close: &str,
    action: CommentAction,
) -> Result<String, CommentError> {
    if text.is_empty() {
        return Err(CommentError::EmptySelection);
    }
    if open.is_empty() || close.is_empty() {
        return Err(CommentError::NoCommentSyntax);
    }
    match action {
        CommentAction::Toggle => Ok(toggle_block_comment(text, open, close)),
        CommentAction::Add => Ok(format!("{open} {text} {close}")),
        CommentAction::Remove => {
            let trimmed = text.trim();
            if trimmed.starts_with(open) && trimmed.ends_with(close) {
                Ok(remove_block_comment(text, open, close))
            } else {
                Ok(text.to_string())
            }
        }
    }
}

/// Count how many lines in `lines` are commented with `prefix`.
pub fn count_commented_lines(lines: &[&str], prefix: &str) -> usize {
    lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .filter(|l| is_line_commented(l, prefix))
        .count()
}

/// Remove ALL line comments from every line in `lines`.
///
/// Each occurrence of `prefix` (plus one optional trailing space) at the
/// start of the non-whitespace portion of a line is stripped.  Empty lines
/// are left untouched.
pub fn strip_all_comments(lines: &[&str], prefix: &str) -> Vec<String> {
    lines.iter().map(|l| remove_line_prefix(l, prefix)).collect()
}

/// Tracks whether the current position is inside a block comment.
///
/// Feed characters or whole lines to the detector and query
/// [`is_inside`](CommentDetector::is_inside) at any point.
#[derive(Debug, Clone)]
pub struct CommentDetector {
    open: String,
    close: String,
    depth: usize,
}

impl CommentDetector {
    /// Create a new detector for the given open/close markers.
    pub fn new(open: &str, close: &str) -> Self {
        Self {
            open: open.to_string(),
            close: close.to_string(),
            depth: 0,
        }
    }

    /// Feed a line (or arbitrary text) to the detector, updating its state.
    pub fn feed(&mut self, text: &str) {
        let mut pos = 0;
        let bytes = text.as_bytes();
        while pos < bytes.len() {
            if text[pos..].starts_with(&self.open) {
                self.depth += 1;
                pos += self.open.len();
            } else if self.depth > 0 && text[pos..].starts_with(&self.close) {
                self.depth -= 1;
                pos += self.close.len();
            } else {
                pos += 1;
            }
        }
    }

    /// Returns `true` if the detector is currently inside a block comment.
    pub fn is_inside(&self) -> bool {
        self.depth > 0
    }

    /// Returns the current nesting depth.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Reset the detector to the initial (outside) state.
    pub fn reset(&mut self) {
        self.depth = 0;
    }
}

// ── comment statistics ─────────────────────────────────────────────────

/// Statistics about comment coverage in a set of lines.
#[derive(Debug, Clone, PartialEq)]
pub struct CommentStats {
    /// Total number of lines analysed.
    pub total_lines: usize,
    /// Number of lines that carry a line-comment prefix.
    pub commented_lines: usize,
    /// Number of lines that are blank (empty or whitespace-only).
    pub blank_lines: usize,
    /// Ratio of commented lines to non-blank lines (`0.0` when there are
    /// no non-blank lines).
    pub comment_density: f64,
    /// Length (in lines) of the longest contiguous run of commented lines.
    pub longest_commented_block: usize,
}

impl CommentStats {
    /// Ratio of commented lines to total lines (`0.0` when there are no lines).
    pub fn comment_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.commented_lines as f64 / self.total_lines as f64
        }
    }

    /// Returns `true` if more than half of the lines are commented.
    pub fn is_mostly_commented(&self) -> bool {
        self.comment_ratio() > 0.5
    }
}

/// Analyse `lines` for comment coverage using the given line-comment
/// `prefix`.
pub fn compute_comment_stats(lines: &[&str], prefix: &str) -> CommentStats {
    let total_lines = lines.len();
    let mut commented_lines: usize = 0;
    let mut blank_lines: usize = 0;
    let mut longest_commented_block: usize = 0;
    let mut current_run: usize = 0;

    for line in lines {
        if line.trim().is_empty() {
            blank_lines += 1;
            current_run = 0;
        } else if is_line_commented(line, prefix) {
            commented_lines += 1;
            current_run += 1;
            if current_run > longest_commented_block {
                longest_commented_block = current_run;
            }
        } else {
            current_run = 0;
        }
    }

    let non_blank = total_lines - blank_lines;
    let comment_density = if non_blank == 0 {
        0.0
    } else {
        commented_lines as f64 / non_blank as f64
    };

    CommentStats {
        total_lines,
        commented_lines,
        blank_lines,
        comment_density,
        longest_commented_block,
    }
}

// ── comment blocks ────────────────────────────────────────────────────

/// A contiguous block of commented lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentBlock {
    /// Index of the first commented line (inclusive).
    pub start: usize,
    /// Index of the last commented line (inclusive).
    pub end: usize,
    /// Number of lines in the block (`end - start + 1`).
    pub line_count: usize,
}

/// Find every contiguous block of lines that start with `prefix`.
///
/// Blank lines break a block.  Returns an empty `Vec` when no commented
/// lines are found.
pub fn find_comment_blocks(lines: &[&str], prefix: &str) -> Vec<CommentBlock> {
    let mut blocks = Vec::new();
    let mut block_start: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let is_commented = !line.trim().is_empty() && is_line_commented(line, prefix);
        if is_commented {
            if block_start.is_none() {
                block_start = Some(i);
            }
        } else if let Some(start) = block_start.take() {
            blocks.push(CommentBlock {
                start,
                end: i - 1,
                line_count: i - start,
            });
        }
    }
    // Close a trailing block.
    if let Some(start) = block_start {
        blocks.push(CommentBlock {
            start,
            end: lines.len() - 1,
            line_count: lines.len() - start,
        });
    }
    blocks
}

/// Remove the comment prefix from the **first** contiguous commented
/// block found in `lines`.
///
/// Returns the transformed lines together with the [`CommentBlock`] that
/// was uncommented (or `None` if no commented block exists).
pub fn uncomment_first_block(
    lines: &[&str],
    prefix: &str,
) -> (Vec<String>, Option<CommentBlock>) {
    let blocks = find_comment_blocks(lines, prefix);
    let block = match blocks.into_iter().next() {
        Some(b) => b,
        None => return (lines.iter().map(|l| l.to_string()).collect(), None),
    };

    let mut result: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if i >= block.start && i <= block.end {
            result.push(remove_line_prefix(line, prefix));
        } else {
            result.push(line.to_string());
        }
    }
    (result, Some(block))
}

// ── indent-aware block comment ────────────────────────────────────────

/// Wrap `lines` in a block comment while preserving relative indentation.
///
/// The opening marker is placed on its own line at the minimum indentation
/// level found across all non-blank lines, and the closing marker likewise.
/// Each original line is emitted unchanged between the markers.
pub fn indent_aware_block_comment(
    lines: &[&str],
    open: &str,
    close: &str,
) -> Vec<String> {
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    let pad: String = " ".repeat(min_indent);
    let mut result = Vec::with_capacity(lines.len() + 2);
    result.push(format!("{pad}{open}"));
    for line in lines {
        result.push(line.to_string());
    }
    result.push(format!("{pad}{close}"));
    result
}

// ── comment style detector ────────────────────────────────────────────

/// The comment style detected in a piece of source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedCommentStyle {
    /// Lines using a single-line prefix (e.g. `//`, `#`, `--`).
    Line(String),
    /// Blocks delimited by an open/close pair (e.g. `/* … */`,
    /// `<!-- … -->`).
    Block { open: String, close: String },
}

/// Heuristic detector that guesses the comment style used in a body of
/// source code.
pub struct CommentStyleDetector;

impl CommentStyleDetector {
    /// Well-known line-comment prefixes, ordered by specificity.
    const LINE_PREFIXES: &[&str] = &["//", "#", "--", ";", "%"];
    /// Well-known block-comment pairs.
    const BLOCK_PAIRS: &[(&str, &str)] = &[("/*", "*/"), ("<!--", "-->"), ("{-", "-}")];

    /// Scan `content` and return the most likely comment style, or `None`
    /// if no comment markers are found.
    pub fn detect(content: &str) -> Option<DetectedCommentStyle> {
        // Count occurrences of each line-comment prefix.
        let mut best_line: Option<(&str, usize)> = None;
        for &pfx in Self::LINE_PREFIXES {
            let count = content
                .lines()
                .filter(|l| l.trim_start().starts_with(pfx))
                .count();
            if count > 0 {
                if best_line.map_or(true, |(_, c)| count > c) {
                    best_line = Some((pfx, count));
                }
            }
        }

        // Check for block comment markers.
        let mut best_block: Option<(&str, &str, usize)> = None;
        for &(open, close) in Self::BLOCK_PAIRS {
            let opens = content.matches(open).count();
            let closes = content.matches(close).count();
            let count = opens.min(closes);
            if count > 0 {
                if best_block.map_or(true, |(_, _, c)| count > c) {
                    best_block = Some((open, close, count));
                }
            }
        }

        // Prefer whichever style has more evidence.
        match (best_line, best_block) {
            (Some((pfx, lc)), Some((open, close, bc))) => {
                if lc >= bc {
                    Some(DetectedCommentStyle::Line(pfx.to_string()))
                } else {
                    Some(DetectedCommentStyle::Block {
                        open: open.to_string(),
                        close: close.to_string(),
                    })
                }
            }
            (Some((pfx, _)), None) => Some(DetectedCommentStyle::Line(pfx.to_string())),
            (None, Some((open, close, _))) => Some(DetectedCommentStyle::Block {
                open: open.to_string(),
                close: close.to_string(),
            }),
            (None, None) => None,
        }
    }
}

// ── comment formatter ─────────────────────────────────────────────────

/// Normalizes comment formatting by ensuring consistent spacing after
/// the comment prefix.
pub struct CommentFormatter;

impl CommentFormatter {
    /// Normalize each line so that `prefix` is followed by exactly one
    /// space. Lines that are not commented or are blank are left as-is.
    pub fn normalize_spacing(lines: &[&str], prefix: &str) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                let trimmed = line.trim_start();
                if trimmed.is_empty() || !trimmed.starts_with(prefix) {
                    return line.to_string();
                }
                let indent_len = line.len() - trimmed.len();
                let indent = &line[..indent_len];
                let after_prefix = &trimmed[prefix.len()..];
                let content = after_prefix.trim_start();
                if content.is_empty() {
                    format!("{indent}{prefix}")
                } else {
                    format!("{indent}{prefix} {content}")
                }
            })
            .collect()
    }

    /// Return `true` if every commented line already has exactly one space
    /// after the prefix.
    pub fn is_normalized(lines: &[&str], prefix: &str) -> bool {
        for line in lines {
            let trimmed = line.trim_start();
            if !trimmed.starts_with(prefix) || trimmed.is_empty() {
                continue;
            }
            let after = &trimmed[prefix.len()..];
            if after.is_empty() {
                continue;
            }
            if !after.starts_with(' ') || after.starts_with("  ") {
                return false;
            }
        }
        true
    }
}

// ── per-line block comment toggle ─────────────────────────────────────

/// Toggle block comments on a multi-line selection, operating per-line.
///
/// If **all** non-empty lines are wrapped in `open…close`, the markers are
/// removed from each line. Otherwise, each non-empty line is wrapped with
/// `open…close`. Empty lines are left untouched.
pub fn block_comment_toggle(lines: &[&str], open: &str, close: &str) -> Vec<String> {
    let all_wrapped = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .all(|l| {
            let trimmed = l.trim();
            trimmed.starts_with(open) && trimmed.ends_with(close)
        });

    lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                return line.to_string();
            }
            if all_wrapped {
                let indent_len = line.len() - line.trim_start().len();
                let (indent, rest) = line.split_at(indent_len);
                let trimmed = rest.trim_end();
                let inner = &trimmed[open.len()..trimmed.len() - close.len()];
                let inner = inner.strip_prefix(' ').unwrap_or(inner);
                let inner = inner.strip_suffix(' ').unwrap_or(inner);
                format!("{indent}{inner}")
            } else {
                let indent_len = line.len() - line.trim_start().len();
                let (indent, rest) = line.split_at(indent_len);
                format!("{indent}{open} {rest} {close}")
            }
        })
        .collect()
}

// ── comment style report ──────────────────────────────────────────────

/// Full report of comment styles found in source text.
#[derive(Debug, Clone, PartialEq)]
pub struct CommentStyleReport {
    /// Line-comment prefixes with their occurrence counts, sorted by count
    /// descending.
    pub line_styles: Vec<(String, usize)>,
    /// Block-comment pairs with their occurrence counts, sorted by count
    /// descending.
    pub block_styles: Vec<(String, String, usize)>,
    /// The dominant comment style, if any.
    pub dominant_style: Option<DetectedCommentStyle>,
    /// Total number of lines that contain a recognised comment marker.
    pub total_comment_lines: usize,
    /// Total number of lines in the input.
    pub total_lines: usize,
}

/// Analyze source code to identify all comment patterns and their
/// frequencies.
pub fn detect_comment_style(content: &str) -> CommentStyleReport {
    let total_lines = content.lines().count();

    let mut line_styles: Vec<(String, usize)> = Vec::new();
    for &pfx in CommentStyleDetector::LINE_PREFIXES {
        let count = content
            .lines()
            .filter(|l| l.trim_start().starts_with(pfx))
            .count();
        if count > 0 {
            line_styles.push((pfx.to_string(), count));
        }
    }
    line_styles.sort_by(|a, b| b.1.cmp(&a.1));

    let mut block_styles: Vec<(String, String, usize)> = Vec::new();
    for &(open, close) in CommentStyleDetector::BLOCK_PAIRS {
        let opens = content.matches(open).count();
        let closes = content.matches(close).count();
        let count = opens.min(closes);
        if count > 0 {
            block_styles.push((open.to_string(), close.to_string(), count));
        }
    }
    block_styles.sort_by(|a, b| b.2.cmp(&a.2));

    let total_comment_lines = content
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            CommentStyleDetector::LINE_PREFIXES.iter().any(|p| t.starts_with(p))
                || CommentStyleDetector::BLOCK_PAIRS
                    .iter()
                    .any(|(o, c)| t.contains(o) || t.contains(c))
        })
        .count();

    let dominant_style = CommentStyleDetector::detect(content);

    CommentStyleReport {
        line_styles,
        block_styles,
        dominant_style,
        total_comment_lines,
        total_lines,
    }
}

// ── multi-line block comment helper ───────────────────────────────────

/// Helper for wrapping and unwrapping text in block-comment markers.
#[derive(Debug, Clone)]
pub struct MultiLineBlockComment {
    open: String,
    close: String,
}

impl MultiLineBlockComment {
    /// Create a new helper with the given open/close markers.
    pub fn new(open: &str, close: &str) -> Self {
        Self {
            open: open.to_string(),
            close: close.to_string(),
        }
    }

    /// Wrap `text` in the block-comment markers with surrounding spaces.
    pub fn wrap(&self, text: &str) -> String {
        format!("{} {} {}", self.open, text, self.close)
    }

    /// Remove block-comment markers if present, returning `None` if the
    /// text is not wrapped.
    pub fn unwrap(&self, text: &str) -> Option<String> {
        let trimmed = text.trim();
        if self.is_wrapped(text) {
            let inner = &trimmed[self.open.len()..trimmed.len() - self.close.len()];
            let inner = inner.strip_prefix(' ').unwrap_or(inner);
            let inner = inner.strip_suffix(' ').unwrap_or(inner);
            Some(inner.to_string())
        } else {
            None
        }
    }

    /// Return `true` if `text` is wrapped in the block-comment markers.
    pub fn is_wrapped(&self, text: &str) -> bool {
        let trimmed = text.trim();
        trimmed.starts_with(&self.open) && trimmed.ends_with(&self.close)
    }

    /// Toggle block-comment markers: wrap if not wrapped, unwrap if
    /// already wrapped.
    pub fn toggle(&self, text: &str) -> String {
        if let Some(inner) = self.unwrap(text) {
            inner
        } else {
            self.wrap(text)
        }
    }
}

/// Remove line-comment prefixes from every line in `lines`.
///
/// Each occurrence of `prefix` (plus one optional trailing space) at the
/// start of the non-whitespace portion of a line is stripped. Empty lines
/// and lines without the prefix are left untouched.
pub fn uncomment_all_lines(lines: &[&str], prefix: &str) -> Vec<String> {
    lines.iter().map(|l| remove_line_prefix(l, prefix)).collect()
}

/// Wrap `text` in block-comment markers.
pub fn wrap_in_block_comment(text: &str, open: &str, close: &str) -> String {
    format!("{open} {text} {close}")
}

// ── tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_line_comments() {
        let lines = vec!["fn main() {", "    println!(\"hi\");", "}"];
        let result = toggle_line_comment(&lines, "//");
        assert_eq!(result, vec!["// fn main() {", "    // println!(\"hi\");", "// }"]);
    }

    #[test]
    fn remove_line_comments() {
        let lines = vec!["// fn main() {", "    // println!(\"hi\");", "// }"];
        let result = toggle_line_comment(&lines, "//");
        assert_eq!(result, vec!["fn main() {", "    println!(\"hi\");", "}"]);
    }

    #[test]
    fn toggle_block_comment_add_and_remove() {
        let text = "some code";
        let wrapped = toggle_block_comment(text, "/*", "*/");
        assert_eq!(wrapped, "/* some code */");

        let unwrapped = toggle_block_comment(&wrapped, "/*", "*/");
        assert_eq!(unwrapped, "some code");
    }

    #[test]
    fn empty_lines_are_preserved() {
        let lines = vec!["// a", "", "// b"];
        let result = toggle_line_comment(&lines, "//");
        assert_eq!(result, vec!["a", "", "b"]);
    }

    #[test]
    fn is_line_commented_detects_prefix() {
        assert!(is_line_commented("  // hello", "//"));
        assert!(!is_line_commented("  hello", "//"));
    }

    // ── new tests ─────────────────────────────────────────────────────

    #[test]
    fn for_language_known() {
        let rust = CommentRule::for_language("rust").unwrap();
        assert_eq!(rust.line_comment.as_deref(), Some("//"));
        assert_eq!(
            rust.block_comment,
            Some(("/*".into(), "*/".into()))
        );

        let py = CommentRule::for_language("python").unwrap();
        assert_eq!(py.line_comment.as_deref(), Some("#"));
        assert!(py.block_comment.is_none());

        let html = CommentRule::for_language("html").unwrap();
        assert!(html.line_comment.is_none());
        assert_eq!(
            html.block_comment,
            Some(("<!--".into(), "-->".into()))
        );
    }

    #[test]
    fn for_language_unknown_returns_none() {
        assert!(CommentRule::for_language("brainfuck").is_none());
        assert!(CommentRule::for_language("").is_none());
    }

    #[test]
    fn apply_comment_action_add() {
        let lines = vec!["a", "b"];
        let result = apply_comment_action(&lines, "//", CommentAction::Add).unwrap();
        assert_eq!(result, vec!["// a", "// b"]);
    }

    #[test]
    fn apply_comment_action_remove() {
        let lines = vec!["// a", "// b"];
        let result = apply_comment_action(&lines, "//", CommentAction::Remove).unwrap();
        assert_eq!(result, vec!["a", "b"]);
    }

    #[test]
    fn apply_comment_action_toggle() {
        let lines = vec!["a", "b"];
        let toggled = apply_comment_action(&lines, "#", CommentAction::Toggle).unwrap();
        assert_eq!(toggled, vec!["# a", "# b"]);
        let refs: Vec<&str> = toggled.iter().map(|s| s.as_str()).collect();
        let back = apply_comment_action(&refs, "#", CommentAction::Toggle).unwrap();
        assert_eq!(back, vec!["a", "b"]);
    }

    #[test]
    fn apply_comment_action_errors() {
        let empty: Vec<&str> = vec![];
        assert_eq!(
            apply_comment_action(&empty, "//", CommentAction::Add),
            Err(CommentError::EmptySelection)
        );
        assert_eq!(
            apply_comment_action(&["a"], "", CommentAction::Add),
            Err(CommentError::NoCommentSyntax)
        );
    }

    #[test]
    fn apply_block_comment_action_add_remove() {
        let added = apply_block_comment_action("code", "/*", "*/", CommentAction::Add).unwrap();
        assert_eq!(added, "/* code */");

        let removed =
            apply_block_comment_action(&added, "/*", "*/", CommentAction::Remove).unwrap();
        assert_eq!(removed, "code");
    }

    #[test]
    fn apply_block_comment_action_errors() {
        assert_eq!(
            apply_block_comment_action("", "/*", "*/", CommentAction::Add),
            Err(CommentError::EmptySelection)
        );
        assert_eq!(
            apply_block_comment_action("x", "", "*/", CommentAction::Add),
            Err(CommentError::NoCommentSyntax)
        );
    }

    #[test]
    fn count_commented_lines_works() {
        let lines = vec!["// a", "b", "// c", "", "// d"];
        assert_eq!(count_commented_lines(&lines, "//"), 3);
        assert_eq!(count_commented_lines(&lines, "#"), 0);
    }

    #[test]
    fn strip_all_comments_works() {
        let lines = vec!["// a", "  // b", "c", ""];
        let result = strip_all_comments(&lines, "//");
        assert_eq!(result, vec!["a", "  b", "c", ""]);
    }

    #[test]
    fn comment_detector_basic() {
        let mut det = CommentDetector::new("/*", "*/");
        assert!(!det.is_inside());

        det.feed("before /* inside");
        assert!(det.is_inside());

        det.feed("still inside */ outside");
        assert!(!det.is_inside());
    }

    #[test]
    fn comment_detector_nested_and_reset() {
        let mut det = CommentDetector::new("/*", "*/");
        det.feed("/* /* nested");
        assert!(det.is_inside());

        det.feed("*/");
        // Still inside because of nesting.
        assert!(det.is_inside());

        det.feed("*/");
        assert!(!det.is_inside());

        // Feed another open, then reset.
        det.feed("/* open");
        assert!(det.is_inside());
        det.reset();
        assert!(!det.is_inside());
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", CommentMode::Line), "line");
        assert_eq!(format!("{}", CommentMode::Block), "block");
        assert_eq!(format!("{}", CommentAction::Toggle), "toggle");
        assert_eq!(format!("{}", CommentAction::Add), "add");
        assert_eq!(format!("{}", CommentAction::Remove), "remove");
    }

    #[test]
    fn error_display() {
        assert_eq!(
            format!("{}", CommentError::NoCommentSyntax),
            "no comment syntax available for this language"
        );
        assert_eq!(format!("{}", CommentError::InvalidRange), "invalid range");
        assert_eq!(format!("{}", CommentError::EmptySelection), "empty selection");
    }

    // ── new tests for added functionality ─────────────────────────────

    #[test]
    fn compute_comment_stats_basic() {
        let lines = vec!["// a", "b", "// c", "", "// d", "// e"];
        let stats = compute_comment_stats(&lines, "//");
        assert_eq!(stats.total_lines, 6);
        assert_eq!(stats.commented_lines, 4);
        assert_eq!(stats.blank_lines, 1);
        assert_eq!(stats.longest_commented_block, 2); // "// d", "// e"
        // 4 commented out of 5 non-blank = 0.8
        assert!((stats.comment_density - 0.8).abs() < 1e-9);
    }

    #[test]
    fn compute_comment_stats_all_blank() {
        let lines: Vec<&str> = vec!["", "   ", ""];
        let stats = compute_comment_stats(&lines, "#");
        assert_eq!(stats.total_lines, 3);
        assert_eq!(stats.commented_lines, 0);
        assert_eq!(stats.blank_lines, 3);
        assert!((stats.comment_density - 0.0).abs() < 1e-9);
        assert_eq!(stats.longest_commented_block, 0);
    }

    #[test]
    fn find_comment_blocks_multiple() {
        let lines = vec!["// a", "// b", "code", "// c", "", "// d"];
        let blocks = find_comment_blocks(&lines, "//");
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0], CommentBlock { start: 0, end: 1, line_count: 2 });
        assert_eq!(blocks[1], CommentBlock { start: 3, end: 3, line_count: 1 });
        assert_eq!(blocks[2], CommentBlock { start: 5, end: 5, line_count: 1 });
    }

    #[test]
    fn find_comment_blocks_none() {
        let lines = vec!["a", "b", "c"];
        let blocks = find_comment_blocks(&lines, "//");
        assert!(blocks.is_empty());
    }

    #[test]
    fn uncomment_first_block_works() {
        let lines = vec!["code", "# x", "# y", "more"];
        let (result, block) = uncomment_first_block(&lines, "#");
        assert_eq!(block, Some(CommentBlock { start: 1, end: 2, line_count: 2 }));
        assert_eq!(result, vec!["code", "x", "y", "more"]);
    }

    #[test]
    fn uncomment_first_block_no_comments() {
        let lines = vec!["a", "b"];
        let (result, block) = uncomment_first_block(&lines, "//");
        assert!(block.is_none());
        assert_eq!(result, vec!["a", "b"]);
    }

    #[test]
    fn indent_aware_block_comment_preserves_indent() {
        let lines = vec!["    fn foo() {", "        bar();", "    }"];
        let result = indent_aware_block_comment(&lines, "/*", "*/");
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], "    /*");
        assert_eq!(result[1], "    fn foo() {");
        assert_eq!(result[2], "        bar();");
        assert_eq!(result[3], "    }");
        assert_eq!(result[4], "    */");
    }

    #[test]
    fn indent_aware_block_comment_no_indent() {
        let lines = vec!["hello", "world"];
        let result = indent_aware_block_comment(&lines, "<!--", "-->");
        assert_eq!(result, vec!["<!--", "hello", "world", "-->"]);
    }

    #[test]
    fn comment_style_detector_line() {
        let content = "// first\n// second\ncode\n// third\n";
        let style = CommentStyleDetector::detect(content);
        assert_eq!(style, Some(DetectedCommentStyle::Line("//".to_string())));
    }

    #[test]
    fn comment_style_detector_block() {
        let content = "/* block one */ code /* block two */\n";
        let style = CommentStyleDetector::detect(content);
        assert_eq!(
            style,
            Some(DetectedCommentStyle::Block {
                open: "/*".to_string(),
                close: "*/".to_string(),
            })
        );
    }

    #[test]
    fn comment_style_detector_none() {
        let content = "just plain code\nno comments here\n";
        assert_eq!(CommentStyleDetector::detect(content), None);
    }

    #[test]
    fn comment_style_detector_hash() {
        let content = "# comment\ncode\n# another\n# more\n";
        let style = CommentStyleDetector::detect(content);
        assert_eq!(style, Some(DetectedCommentStyle::Line("#".to_string())));
    }

    // ── CommentFormatter tests ───────────────────────────────────────

    #[test]
    fn formatter_normalize_spacing() {
        let lines = vec!["//hello", "//  world", "  //foo", "plain", ""];
        let result = CommentFormatter::normalize_spacing(&lines, "//");
        assert_eq!(result[0], "// hello");
        assert_eq!(result[1], "// world");
        assert_eq!(result[2], "  // foo");
        assert_eq!(result[3], "plain");
        assert_eq!(result[4], "");
    }

    #[test]
    fn formatter_already_normalized() {
        let lines = vec!["// hello", "// world"];
        assert!(CommentFormatter::is_normalized(&lines, "//"));
    }

    #[test]
    fn formatter_not_normalized() {
        let lines = vec!["//hello", "// ok"];
        assert!(!CommentFormatter::is_normalized(&lines, "//"));
    }

    #[test]
    fn formatter_double_space_not_normalized() {
        let lines = vec!["//  double"];
        assert!(!CommentFormatter::is_normalized(&lines, "//"));
    }

    #[test]
    fn formatter_empty_comment_stays() {
        let lines = vec!["//"];
        let result = CommentFormatter::normalize_spacing(&lines, "//");
        assert_eq!(result[0], "//");
    }

    // ── block_comment_toggle tests ────────────────────────────────────

    #[test]
    fn block_comment_toggle_adds_per_line() {
        let lines = vec!["fn main() {", "    println!(\"hi\");", "}"];
        let result = block_comment_toggle(&lines, "/*", "*/");
        assert_eq!(
            result,
            vec!["/* fn main() { */", "    /* println!(\"hi\"); */", "/* } */"]
        );
    }

    #[test]
    fn block_comment_toggle_removes_per_line() {
        let lines = vec!["/* fn main() { */", "    /* println!(\"hi\"); */", "/* } */"];
        let result = block_comment_toggle(&lines, "/*", "*/");
        assert_eq!(result, vec!["fn main() {", "    println!(\"hi\");", "}"]);
    }

    #[test]
    fn block_comment_toggle_preserves_empty_lines() {
        let lines = vec!["hello", "", "world"];
        let result = block_comment_toggle(&lines, "/*", "*/");
        assert_eq!(result, vec!["/* hello */", "", "/* world */"]);
    }

    // ── detect_comment_style tests ────────────────────────────────────

    #[test]
    fn detect_comment_style_identifies_line_comments() {
        let content = "// first\n// second\nlet x = 1;\n";
        let report = detect_comment_style(content);
        assert_eq!(
            report.dominant_style,
            Some(DetectedCommentStyle::Line("//".to_string()))
        );
    }

    #[test]
    fn detect_comment_style_identifies_block_comments() {
        let content = "/* block one */\ncode\n/* block two */\n";
        let report = detect_comment_style(content);
        assert_eq!(
            report.dominant_style,
            Some(DetectedCommentStyle::Block {
                open: "/*".to_string(),
                close: "*/".to_string(),
            })
        );
    }

    #[test]
    fn detect_comment_style_reports_counts() {
        let content = "// a\n// b\n// c\nlet x = 1;\nlet y = 2;\n";
        let report = detect_comment_style(content);
        assert_eq!(report.total_lines, 5);
        assert_eq!(report.total_comment_lines, 3);
        assert_eq!(report.line_styles[0], ("//".to_string(), 3));
    }

    // ── MultiLineBlockComment tests ───────────────────────────────────

    #[test]
    fn multi_line_block_comment_wrap_and_unwrap() {
        let bc = MultiLineBlockComment::new("/*", "*/");
        let wrapped = bc.wrap("hello");
        assert_eq!(wrapped, "/* hello */");
        let unwrapped = bc.unwrap(&wrapped);
        assert_eq!(unwrapped, Some("hello".to_string()));
        assert_eq!(bc.unwrap("no markers"), None);
    }

    #[test]
    fn multi_line_block_comment_is_wrapped() {
        let bc = MultiLineBlockComment::new("<!--", "-->");
        assert!(bc.is_wrapped("<!-- stuff -->"));
        assert!(!bc.is_wrapped("not wrapped"));
    }

    #[test]
    fn multi_line_block_comment_toggle() {
        let bc = MultiLineBlockComment::new("/*", "*/");
        assert_eq!(bc.toggle("hello"), "/* hello */");
        assert_eq!(bc.toggle("/* hello */"), "hello");
    }

    // ── tests for newly added functionality ───────────────────────────

    #[test]
    fn comment_rule_supports_block() {
        let rust = CommentRule::for_language("rust").unwrap();
        assert!(rust.supports_block());
        let python = CommentRule::for_language("python").unwrap();
        assert!(!python.supports_block());
    }

    #[test]
    fn comment_rule_supports_line() {
        let rust = CommentRule::for_language("rust").unwrap();
        assert!(rust.supports_line());
        let html = CommentRule::for_language("html").unwrap();
        assert!(!html.supports_line());
    }

    #[test]
    fn comment_detector_depth() {
        let mut det = CommentDetector::new("/*", "*/");
        assert_eq!(det.depth(), 0);
        det.feed("/* outer /* inner");
        assert_eq!(det.depth(), 2);
        det.feed("*/");
        assert_eq!(det.depth(), 1);
        det.feed("*/");
        assert_eq!(det.depth(), 0);
    }

    #[test]
    fn comment_stats_ratio_and_mostly_commented() {
        // 4 commented out of 6 total → ratio ~0.667 > 0.5
        let lines = vec!["// a", "b", "// c", "", "// d", "// e"];
        let stats = compute_comment_stats(&lines, "//");
        assert!((stats.comment_ratio() - 4.0 / 6.0).abs() < 1e-9);
        assert!(stats.is_mostly_commented());

        // 1 commented out of 4 total → ratio 0.25 ≤ 0.5
        let lines2 = vec!["a", "// b", "c", "d"];
        let stats2 = compute_comment_stats(&lines2, "//");
        assert!(!stats2.is_mostly_commented());
    }

    #[test]
    fn comment_stats_empty_lines() {
        let empty: Vec<&str> = vec![];
        let stats = compute_comment_stats(&empty, "//");
        assert!((stats.comment_ratio() - 0.0).abs() < 1e-9);
        assert!(!stats.is_mostly_commented());
    }

    #[test]
    fn uncomment_all_lines_removes_prefix() {
        let lines = vec!["// a", "  // b", "c", ""];
        let result = uncomment_all_lines(&lines, "//");
        assert_eq!(result, vec!["a", "  b", "c", ""]);
    }

    #[test]
    fn wrap_in_block_comment_wraps_text() {
        assert_eq!(wrap_in_block_comment("hello", "/*", "*/"), "/* hello */");
        assert_eq!(wrap_in_block_comment("content", "<!--", "-->"), "<!-- content -->");
    }

    #[test]
    fn comment_mode_label() {
        assert_eq!(CommentMode::Line.label(), "line");
        assert_eq!(CommentMode::Block.label(), "block");
    }
}
