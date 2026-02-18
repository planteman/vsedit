//! VS Code–style line and block comment toggling.
//!
//! Provides enums and helpers to add, remove, or toggle line and block
//! comments for any language whose comment syntax is described by a
//! [`CommentRule`].

/// Whether a comment operation targets lines or blocks.
use std::fmt;
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

// ---------------------------------------------------------------------------
// CommentAligner
// ---------------------------------------------------------------------------

/// Aligns trailing comments in a block of lines to the same column.
pub struct CommentAligner;

impl CommentAligner {
    /// Align trailing line comments so they all start at the same column.
    ///
    /// Lines without the `prefix` are returned unchanged.
    pub fn align_trailing(lines: &[&str], prefix: &str) -> Vec<String> {
        // Find which lines have trailing comments and determine the max code width.
        let parsed: Vec<(String, Option<String>)> = lines
            .iter()
            .map(|line| {
                if let Some(idx) = line.find(prefix) {
                    if idx > 0 {
                        let code = line[..idx].trim_end().to_string();
                        let comment = line[idx..].to_string();
                        return (code, Some(comment));
                    }
                }
                (line.to_string(), None)
            })
            .collect();

        let max_code_width = parsed
            .iter()
            .filter_map(|(code, comment)| comment.as_ref().map(|_| code.len()))
            .max()
            .unwrap_or(0);

        parsed
            .into_iter()
            .map(|(code, comment)| match comment {
                Some(c) => {
                    let padding = max_code_width.saturating_sub(code.len());
                    format!("{}{} {}", code, " ".repeat(padding), c.trim_start())
                }
                None => code,
            })
            .collect()
    }

    /// Count how many lines contain a trailing comment.
    pub fn count_trailing_comments(lines: &[&str], prefix: &str) -> usize {
        lines.iter().filter(|line| {
            if let Some(idx) = line.find(prefix) {
                idx > 0
            } else {
                false
            }
        }).count()
    }
}

// ---------------------------------------------------------------------------
// CommentExtractor
// ---------------------------------------------------------------------------

/// Extracted comment with its location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedComment {
    pub line_number: usize,
    pub text: String,
    pub is_line_comment: bool,
}

/// Extracts all comments from source text.
pub struct CommentExtractor;

impl CommentExtractor {
    /// Extract all line comments from text using the given prefix.
    pub fn extract_line_comments(text: &str, prefix: &str) -> Vec<ExtractedComment> {
        let mut results = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with(prefix) {
                let content = trimmed[prefix.len()..].trim().to_string();
                results.push(ExtractedComment {
                    line_number: i,
                    text: content,
                    is_line_comment: true,
                });
            } else if let Some(idx) = line.find(prefix) {
                if idx > 0 {
                    let content = line[idx + prefix.len()..].trim().to_string();
                    results.push(ExtractedComment {
                        line_number: i,
                        text: content,
                        is_line_comment: true,
                    });
                }
            }
        }
        results
    }

    /// Extract block comments from text given open/close markers.
    pub fn extract_block_comments(text: &str, open: &str, close: &str) -> Vec<ExtractedComment> {
        let mut results = Vec::new();
        let mut search_from = 0;
        let lines: Vec<&str> = text.lines().collect();
        while let Some(start) = text[search_from..].find(open) {
            let abs_start = search_from + start;
            let content_start = abs_start + open.len();
            if let Some(end) = text[content_start..].find(close) {
                let content = text[content_start..content_start + end].trim().to_string();
                let line_number = text[..abs_start].lines().count().saturating_sub(1).min(lines.len().saturating_sub(1));
                results.push(ExtractedComment {
                    line_number,
                    text: content,
                    is_line_comment: false,
                });
                search_from = content_start + end + close.len();
            } else {
                break;
            }
        }
        results
    }

    /// Return the total number of comments (line + block).
    pub fn count_all(text: &str, line_prefix: &str, block_open: &str, block_close: &str) -> usize {
        Self::extract_line_comments(text, line_prefix).len()
            + Self::extract_block_comments(text, block_open, block_close).len()
    }
}

// ---------------------------------------------------------------------------
// CommentWrapper
// ---------------------------------------------------------------------------

/// Wraps long comment lines at a given column width.
pub struct CommentWrapper;

impl CommentWrapper {
    /// Wrap a comment string to fit within `max_width` characters per line,
    /// prefixing continuation lines with `prefix`.
    pub fn wrap_comment(text: &str, prefix: &str, max_width: usize) -> Vec<String> {
        if text.is_empty() {
            return vec![format!("{prefix}")];
        }
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return vec![format!("{prefix}")];
        }

        let mut lines = Vec::new();
        let mut current_line = format!("{prefix} ");
        for word in &words {
            if current_line.len() + word.len() + 1 > max_width && current_line.len() > prefix.len() + 1 {
                lines.push(current_line.trim_end().to_string());
                current_line = format!("{prefix} ");
            }
            current_line.push_str(word);
            current_line.push(' ');
        }
        let trimmed = current_line.trim_end().to_string();
        if !trimmed.is_empty() {
            lines.push(trimmed);
        }
        lines
    }

    /// Wrap a multi-line block comment body so each line fits within `max_width`.
    pub fn wrap_block_comment(
        body: &str,
        open: &str,
        close: &str,
        continuation: &str,
        max_width: usize,
    ) -> Vec<String> {
        let words: Vec<&str> = body.split_whitespace().collect();
        if words.is_empty() {
            return vec![format!("{open} {close}")];
        }
        let mut result = Vec::new();
        result.push(open.to_string());
        let mut current_line = format!("{continuation} ");
        for word in &words {
            if current_line.len() + word.len() + 1 > max_width
                && current_line.len() > continuation.len() + 1
            {
                result.push(current_line.trim_end().to_string());
                current_line = format!("{continuation} ");
            }
            current_line.push_str(word);
            current_line.push(' ');
        }
        let trimmed = current_line.trim_end().to_string();
        if !trimmed.is_empty() {
            result.push(trimmed);
        }
        result.push(close.to_string());
        result
    }

    /// Check whether any line in the comment exceeds `max_width`.
    pub fn needs_wrapping(lines: &[&str], max_width: usize) -> bool {
        lines.iter().any(|l| l.len() > max_width)
    }
}

// ── TODO/FIXME/HACK scanner ───────────────────────────────────────────

/// The kind of annotation marker found in a comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationKind {
    Todo,
    Fixme,
    Hack,
    Note,
    Xxx,
}

impl AnnotationKind {
    /// The canonical uppercase tag for this annotation.
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Todo => "TODO",
            Self::Fixme => "FIXME",
            Self::Hack => "HACK",
            Self::Note => "NOTE",
            Self::Xxx => "XXX",
        }
    }
}

impl std::fmt::Display for AnnotationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
    }
}

/// A single annotation found inside a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentAnnotation {
    /// Zero-based line number where the annotation was found.
    pub line_number: usize,
    /// Kind of annotation.
    pub kind: AnnotationKind,
    /// The text following the annotation tag on the same line.
    pub message: String,
}

/// Scans source text for well-known annotation markers inside comments.
pub struct AnnotationScanner;

impl AnnotationScanner {
    const TAGS: &[(&str, AnnotationKind)] = &[
        ("TODO", AnnotationKind::Todo),
        ("FIXME", AnnotationKind::Fixme),
        ("HACK", AnnotationKind::Hack),
        ("NOTE", AnnotationKind::Note),
        ("XXX", AnnotationKind::Xxx),
    ];

    /// Scan `text` and return every annotation found, in document order.
    pub fn scan(text: &str) -> Vec<CommentAnnotation> {
        let mut results = Vec::new();
        for (line_no, line) in text.lines().enumerate() {
            let upper = line.to_uppercase();
            for &(tag, kind) in Self::TAGS {
                if let Some(idx) = upper.find(tag) {
                    let after = &line[idx + tag.len()..];
                    // Accept optional colon or paren after the tag.
                    let stripped = after
                        .strip_prefix(':')
                        .or_else(|| after.strip_prefix('('))
                        .unwrap_or(after)
                        .trim();
                    let msg = stripped
                        .strip_suffix(')')
                        .unwrap_or(stripped)
                        .trim()
                        .to_string();
                    results.push(CommentAnnotation {
                        line_number: line_no,
                        kind,
                        message: msg,
                    });
                    break; // one annotation per line
                }
            }
        }
        results
    }

    /// Return only annotations of a specific `kind`.
    pub fn scan_kind(text: &str, kind: AnnotationKind) -> Vec<CommentAnnotation> {
        Self::scan(text)
            .into_iter()
            .filter(|a| a.kind == kind)
            .collect()
    }

    /// Count annotations grouped by kind.
    pub fn count_by_kind(text: &str) -> Vec<(AnnotationKind, usize)> {
        let annotations = Self::scan(text);
        let mut counts = std::collections::HashMap::<AnnotationKind, usize>::new();
        for a in &annotations {
            *counts.entry(a.kind).or_insert(0) += 1;
        }
        let mut result: Vec<_> = counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }
}

// ── doc-comment generation ────────────────────────────────────────────

/// Generates stub doc-comments from simple function signatures.
pub struct DocCommentGenerator;

impl DocCommentGenerator {
    /// Generate a Rust-style `///` doc-comment stub for a function
    /// signature line.
    ///
    /// Parses parameter names from the signature and produces a skeleton
    /// with `# Arguments` and `# Returns` sections.
    pub fn generate_rust(sig: &str) -> Vec<String> {
        let mut lines = Vec::new();
        let trimmed = sig.trim();

        // Extract function name.
        let fn_name = trimmed
            .strip_prefix("pub ")
            .or_else(|| trimmed.strip_prefix("pub(crate) "))
            .unwrap_or(trimmed)
            .strip_prefix("fn ")
            .and_then(|rest| rest.split('(').next())
            .unwrap_or("unknown")
            .trim();

        lines.push(format!("/// TODO: describe `{fn_name}`."));
        lines.push("///".to_string());

        // Extract parameter list.
        if let Some(open) = trimmed.find('(') {
            let close = trimmed.rfind(')').unwrap_or(trimmed.len());
            let params_str = &trimmed[open + 1..close];
            let params: Vec<&str> = params_str
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty() && *p != "&self" && *p != "&mut self" && *p != "self")
                .collect();
            if !params.is_empty() {
                lines.push("/// # Arguments".to_string());
                lines.push("///".to_string());
                for param in &params {
                    let name = param.split(':').next().unwrap_or(param).trim();
                    lines.push(format!("/// * `{name}` - TODO"));
                }
                lines.push("///".to_string());
            }
        }

        // Check for a return type.
        let has_return = trimmed.contains("->")
            && !trimmed.contains("-> ()")
            && !trimmed.ends_with("-> ()");
        if has_return {
            lines.push("/// # Returns".to_string());
            lines.push("///".to_string());
            lines.push("/// TODO: describe return value.".to_string());
        }

        lines
    }
}

// ── comment conversion ─────────────────────────────────────────────────

/// Convert line comments to a block comment.
///
/// Takes lines with a `prefix` (e.g. `//`) and produces a single block
/// comment string using `open`/`close` markers. Only the comment text is
/// kept; code-only lines are passed through unchanged.
pub fn line_comments_to_block(
    lines: &[&str],
    prefix: &str,
    open: &str,
    close: &str,
) -> Vec<String> {
    let mut result = Vec::new();
    let mut comment_buf: Vec<String> = Vec::new();

    let flush = |buf: &mut Vec<String>, out: &mut Vec<String>, open: &str, close: &str| {
        if !buf.is_empty() {
            let body = buf.join(" ");
            out.push(format!("{open} {body} {close}"));
            buf.clear();
        }
    };

    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with(prefix) {
            let content = trimmed[prefix.len()..].trim();
            if !content.is_empty() {
                comment_buf.push(content.to_string());
            }
        } else {
            flush(&mut comment_buf, &mut result, open, close);
            result.push(line.to_string());
        }
    }
    flush(&mut comment_buf, &mut result, open, close);
    result
}

/// Convert a block comment back to line comments.
///
/// Splits the inner text of a block comment (delimited by `open`/`close`)
/// into individual line comments using `prefix`.
pub fn block_comment_to_lines(text: &str, prefix: &str, open: &str, close: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.starts_with(open) && trimmed.ends_with(close) {
        let inner = &trimmed[open.len()..trimmed.len() - close.len()];
        let inner = inner.trim();
        if inner.is_empty() {
            return vec![format!("{prefix}")];
        }
        inner
            .split('\n')
            .map(|l| {
                let t = l.trim();
                // Strip optional continuation markers like " * "
                let t = t.strip_prefix("* ").unwrap_or(t);
                let t = t.strip_prefix('*').unwrap_or(t).trim();
                if t.is_empty() {
                    format!("{prefix}")
                } else {
                    format!("{prefix} {t}")
                }
            })
            .collect()
    } else {
        vec![text.to_string()]
    }
}

// ── comment indentation helpers ───────────────────────────────────────

/// Compute the minimum indentation (in spaces) across all non-blank lines.
pub fn min_indentation(lines: &[&str]) -> usize {
    lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0)
}

/// Re-indent commented lines so the comment prefix sits at `target_col`.
///
/// Only lines that start with `prefix` (after whitespace) are adjusted.
/// Non-commented and blank lines are returned unchanged.
pub fn reindent_comments(lines: &[&str], prefix: &str, target_col: usize) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with(prefix) {
                let pad: String = " ".repeat(target_col);
                format!("{pad}{trimmed}")
            } else {
                line.to_string()
            }
        })
        .collect()
}

/// Dedent commented lines by removing up to `n` leading spaces from each
/// commented line.
pub fn dedent_comments(lines: &[&str], prefix: &str, n: usize) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with(prefix) {
                let current_indent = line.len() - trimmed.len();
                let new_indent = current_indent.saturating_sub(n);
                let pad: String = " ".repeat(new_indent);
                format!("{pad}{trimmed}")
            } else {
                line.to_string()
            }
        })
        .collect()
}

// ── Rustdoc / JSDoc parser ────────────────────────────────────────────

/// A parsed section from a Rustdoc or JSDoc comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocSection {
    /// Section heading (e.g. "Arguments", "Returns", "Examples").
    /// `None` for the leading summary.
    pub heading: Option<String>,
    /// Body lines of this section (trimmed of comment markers).
    pub body: Vec<String>,
}

/// Parse a Rustdoc comment (lines starting with `///`) into sections.
///
/// Sections are delimited by `# Heading` markers. The initial text before
/// any heading becomes a section with `heading: None`.
pub fn parse_rustdoc_sections(lines: &[&str]) -> Vec<DocSection> {
    let mut sections: Vec<DocSection> = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_body: Vec<String> = Vec::new();

    for line in lines {
        let trimmed = line.trim_start();
        let content = if let Some(rest) = trimmed.strip_prefix("///") {
            rest.strip_prefix(' ').unwrap_or(rest)
        } else {
            continue;
        };

        if let Some(heading_text) = content.strip_prefix("# ") {
            // Flush previous section.
            sections.push(DocSection {
                heading: current_heading.take(),
                body: std::mem::take(&mut current_body),
            });
            current_heading = Some(heading_text.trim().to_string());
        } else {
            current_body.push(content.to_string());
        }
    }

    // Flush trailing section.
    if current_heading.is_some() || !current_body.is_empty() {
        sections.push(DocSection {
            heading: current_heading,
            body: current_body,
        });
    }
    sections
}

/// Parse a JSDoc block comment into sections.
///
/// Recognises `@param`, `@returns`, `@throws`, `@example`, etc.
pub fn parse_jsdoc_sections(text: &str) -> Vec<DocSection> {
    let mut sections: Vec<DocSection> = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_body: Vec<String> = Vec::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        // Strip block comment markers and continuation stars.
        let line = line.strip_prefix("/**").unwrap_or(line);
        let line = line.strip_prefix("*/").unwrap_or(line);
        let line = line.strip_suffix("*/").unwrap_or(line);
        let line = line.strip_prefix("* ").or_else(|| line.strip_prefix('*')).unwrap_or(line);
        let line = line.trim();

        if line.starts_with('@') {
            // Flush previous.
            if current_heading.is_some() || !current_body.is_empty() {
                sections.push(DocSection {
                    heading: current_heading.take(),
                    body: std::mem::take(&mut current_body),
                });
            }
            let tag_end = line.find(' ').unwrap_or(line.len());
            current_heading = Some(line[..tag_end].to_string());
            let rest = line[tag_end..].trim();
            if !rest.is_empty() {
                current_body.push(rest.to_string());
            }
        } else if !line.is_empty() {
            current_body.push(line.to_string());
        }
    }

    if current_heading.is_some() || !current_body.is_empty() {
        sections.push(DocSection {
            heading: current_heading,
            body: current_body,
        });
    }
    sections
}

// ── comment line classifier ───────────────────────────────────────────

/// Classification of a single line with respect to comments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// A blank (empty / whitespace-only) line.
    Blank,
    /// A line that is entirely a comment (possibly with leading whitespace).
    FullComment,
    /// A code line with a trailing inline comment.
    CodeWithTrailingComment,
    /// A line of pure code (no comment marker found).
    Code,
}

/// Classify a line with respect to a given line-comment `prefix`.
pub fn classify_line(line: &str, prefix: &str) -> LineKind {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        LineKind::Blank
    } else if trimmed.starts_with(prefix) {
        LineKind::FullComment
    } else if let Some(idx) = line.find(prefix) {
        // Make sure the prefix is not inside a string literal (simple
        // heuristic: prefix must be preceded by whitespace).
        if idx > 0 && line.as_bytes()[idx - 1].is_ascii_whitespace() {
            LineKind::CodeWithTrailingComment
        } else {
            LineKind::Code
        }
    } else {
        LineKind::Code
    }
}

/// Classify every line, returning a vec of `(line_index, LineKind)`.
pub fn classify_lines(lines: &[&str], prefix: &str) -> Vec<(usize, LineKind)> {
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| (i, classify_line(l, prefix)))
        .collect()
}

// ── strip trailing comments ───────────────────────────────────────────

/// Remove trailing inline comments from lines, keeping only the code part.
///
/// Lines that are entirely a comment or blank are returned unchanged.
pub fn strip_trailing_comments(lines: &[&str], prefix: &str) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            let kind = classify_line(line, prefix);
            match kind {
                LineKind::CodeWithTrailingComment => {
                    if let Some(idx) = line.find(prefix) {
                        line[..idx].trim_end().to_string()
                    } else {
                        line.to_string()
                    }
                }
                _ => line.to_string(),
            }
        })
        .collect()
}

// ── comment region builder ────────────────────────────────────────────

/// Builds a banner-style comment region with optional surrounding blank
/// comment lines and a title.
pub fn build_comment_region(
    title: &str,
    prefix: &str,
    width: usize,
) -> Vec<String> {
    let fill_char = '─';
    let inner_width = width.saturating_sub(prefix.len() + 2); // +2 for space after prefix + space before suffix
    let title_len = title.len() + 2; // spaces around title
    if title_len >= inner_width {
        return vec![format!("{prefix} {title}")];
    }
    let left = (inner_width - title_len) / 2;
    let right = inner_width - title_len - left;
    let bar_left: String = std::iter::repeat(fill_char).take(left).collect();
    let bar_right: String = std::iter::repeat(fill_char).take(right).collect();
    let separator: String = std::iter::repeat(fill_char).take(inner_width).collect();

    vec![
        format!("{prefix} {separator}"),
        format!("{prefix} {bar_left} {title} {bar_right}"),
        format!("{prefix} {separator}"),
    ]
}

// ── tests ──────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// CommentBlockFormatter - comment block formatter
// ---------------------------------------------------------------------------

/// Severity level for comment block formatter issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommentBlockFormatterSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for CommentBlockFormatterSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [CommentBlockFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentBlockFormatterEntry {
    pub id: String,
    pub label: String,
    pub severity: CommentBlockFormatterSeverity,
    pub detail: Option<String>,
    pub line_count: usize,
    enabled: bool,
}

impl CommentBlockFormatterEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: CommentBlockFormatterSeverity::Low,
            detail: None,
            line_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: CommentBlockFormatterSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_line_count(mut self, val: usize) -> Self {
        self.line_count = val;
        self
    }

    pub fn is_commented(&self) -> bool {
        self.enabled && self.severity >= CommentBlockFormatterSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.line_count, det)
    }
}

impl fmt::Display for CommentBlockFormatterEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [CommentBlockFormatterEntry] items.
#[derive(Debug, Clone)]
pub struct CommentBlockFormatter {
    entries: Vec<CommentBlockFormatterEntry>,
    name: String,
    capacity: usize,
}

impl CommentBlockFormatter {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: CommentBlockFormatterEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<CommentBlockFormatterEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&CommentBlockFormatterEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn line_count(&self) -> usize { self.entries.len() }

    pub fn is_commented(&self) -> bool {
        self.entries.iter().any(|e| e.is_commented())
    }

    pub fn entries_by_severity(&self, severity: CommentBlockFormatterSeverity) -> Vec<&CommentBlockFormatterEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= CommentBlockFormatterSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&CommentBlockFormatterEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&CommentBlockFormatterEntry> {
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
// CommentToggle - comment toggle logic
// ---------------------------------------------------------------------------

/// Configuration for [CommentToggle].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentToggleConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub comment_style_count: usize,
}

impl CommentToggleConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, comment_style_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_comment_style_count(mut self, val: usize) -> Self { self.comment_style_count = val; self }
}

impl Default for CommentToggleConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [CommentToggle].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentToggleItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl CommentToggleItem {
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

    pub fn is_block_comment(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for CommentToggleItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [CommentToggleItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct CommentToggle {
    config: CommentToggleConfig,
    items: Vec<CommentToggleItem>,
}

impl CommentToggle {
    pub fn new(config: CommentToggleConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: CommentToggleItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<CommentToggleItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&CommentToggleItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn comment_style_count(&self) -> usize { self.items.len() }

    pub fn is_block_comment(&self) -> bool {
        self.items.iter().any(|i| i.is_block_comment())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&CommentToggleItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&CommentToggleItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &CommentToggleConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



/// Code comment configuration manager.
#[derive(Debug, Clone)]
pub struct CommentConfig {
    entries: Vec<CommentEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single code comment entry.
#[derive(Debug, Clone, PartialEq)]
pub struct CommentEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl CommentEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl CommentConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: CommentEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&CommentEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut CommentEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&CommentEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&CommentEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&CommentEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<CommentEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Comment toggle and block operations — extended utilities (qo)
// ---------------------------------------------------------------------------

/// Metric accumulator for comment operations.
#[derive(Debug, Clone)]
pub struct QoMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QoMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for comment.
#[derive(Debug, Clone)]
pub struct QoRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QoRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for comment lookups.
#[derive(Debug, Clone)]
pub struct QoLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QoLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 2
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer2 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer2 {
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
pub fn xb_fnv1a_2(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_2<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_2<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_2(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_2(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 20
// ---------------------------------------------------------------------------

/// Generic object pool `Xc20Pool<T>`.
pub struct Xc20Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc20Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc20PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc20Pool<T> {
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
    pub fn stats(&self) -> Xc20PoolStats {
        Xc20PoolStats {
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

impl<T> Default for Xc20Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc20Scheduler`.
pub struct Xc20Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc20Scheduler {
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

impl Default for Xc20Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_20 hash for the given byte slice.
pub fn xc_20_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_20 convention.
pub fn xc_20_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe2 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe2Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe2PipelineError {
    pub stage: Xe2Stage,
    pub message: String,
}

impl std::fmt::Display for Xe2PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe2Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe2Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError>>>,
    stage_names: Vec<Xe2Stage>,
}

impl Xe2Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe2Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe2Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe2Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe2Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError> {
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

    pub fn compose(mut self, other: Xe2Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe2CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe2CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe2Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe2CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe2CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe2Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe2CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_2_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe2CacheEntry {
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

    fn xe_2_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe2CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_2_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError> {
    Ok(data)
}

pub fn xe_2_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_2_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_2_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_2_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe2PipelineError> {
    Err(Xe2PipelineError {
        stage: Xe2Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #63
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf63Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf63TrieNode {
    children: std::collections::HashMap<char, Xf63TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf63Trie {
    root: Xf63TrieNode,
    count: usize,
}

impl Xf63Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf63TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf63TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf63TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf63BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf63BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 19).
pub struct Xh19SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh19SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 61 as u64,
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

/// A compact bit set supporting boolean operations (variant 19).
pub struct Xh19BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh19BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 19).
pub struct Xi19Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi19Deque<T> {
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
pub struct Xi19Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi19Interval {
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

/// A simple interval tree (variant 19).
pub struct Xi19IntervalTree {
    xi_intervals: Vec<Xi19Interval>,
}

impl Xi19IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi19Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi19Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi19Interval) -> Vec<&Xi19Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi19Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi19Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi19Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi19Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi19Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi19Interval> = Vec::new();
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

    // ── CommentAligner / Extractor / Wrapper tests ──

    #[test]
    fn aligner_aligns_trailing_comments() {
        let lines = vec![
            "let x = 1; // short",
            "let long_variable = 42; // longer",
            "no_comment_here",
        ];
        let result = CommentAligner::align_trailing(&lines, "//");
        assert!(result[0].contains("// short"));
        assert!(result[1].contains("// longer"));
        // Both comment columns should be at the same position
        let col0 = result[0].find("//").unwrap();
        let col1 = result[1].find("//").unwrap();
        assert_eq!(col0, col1);
    }

    #[test]
    fn extractor_extracts_line_comments() {
        let text = "let x = 1;\n// a comment\nlet y = 2; // inline\n";
        let comments = CommentExtractor::extract_line_comments(text, "//");
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].text, "a comment");
        assert!(comments[0].is_line_comment);
        assert_eq!(comments[1].text, "inline");
    }

    #[test]
    fn extractor_extracts_block_comments() {
        let text = "code /* block one */ more /* block two */ end";
        let comments = CommentExtractor::extract_block_comments(text, "/*", "*/");
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].text, "block one");
        assert_eq!(comments[1].text, "block two");
        assert!(!comments[0].is_line_comment);
    }

    #[test]
    fn wrapper_wraps_long_comment() {
        let text = "this is a very long comment that should be wrapped across multiple lines";
        let lines = CommentWrapper::wrap_comment(text, "//", 30);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.starts_with("//"));
        }
    }

    #[test]
    fn wrapper_needs_wrapping_detects_long_lines() {
        assert!(CommentWrapper::needs_wrapping(&["// short", "// this is a very very long line"], 20));
        assert!(!CommentWrapper::needs_wrapping(&["// ok", "// fine"], 20));
    }

    #[test]
    fn wrapper_block_comment_wrapping() {
        let body = "short text that fits and some more words to wrap around the limit";
        let result = CommentWrapper::wrap_block_comment(body, "/*", "*/", " *", 30);
        assert_eq!(result[0], "/*");
        assert_eq!(*result.last().unwrap(), "*/");
        assert!(result.len() >= 3);
    }

    #[test]
    fn count_trailing_comments() {
        let lines = vec!["a // x", "b", "c // y"];
        assert_eq!(CommentAligner::count_trailing_comments(&lines, "//"), 2);
    }

    // ── AnnotationScanner tests ───────────────────────────────────────

    #[test]
    fn annotation_scanner_finds_todos() {
        let src = "// TODO: fix this\nlet x = 1;\n// FIXME: broken\n";
        let annots = AnnotationScanner::scan(src);
        assert_eq!(annots.len(), 2);
        assert_eq!(annots[0].kind, AnnotationKind::Todo);
        assert_eq!(annots[0].message, "fix this");
        assert_eq!(annots[0].line_number, 0);
        assert_eq!(annots[1].kind, AnnotationKind::Fixme);
        assert_eq!(annots[1].message, "broken");
        assert_eq!(annots[1].line_number, 2);
    }

    #[test]
    fn annotation_scanner_case_insensitive() {
        let src = "// todo: lower\n// Hack: mixed\n";
        let annots = AnnotationScanner::scan(src);
        assert_eq!(annots.len(), 2);
        assert_eq!(annots[0].kind, AnnotationKind::Todo);
        assert_eq!(annots[1].kind, AnnotationKind::Hack);
    }

    #[test]
    fn annotation_scanner_scan_kind_filters() {
        let src = "// TODO a\n// FIXME b\n// TODO c\n";
        let todos = AnnotationScanner::scan_kind(src, AnnotationKind::Todo);
        assert_eq!(todos.len(), 2);
        assert!(todos.iter().all(|a| a.kind == AnnotationKind::Todo));
    }

    #[test]
    fn annotation_scanner_count_by_kind() {
        let src = "// TODO a\n// TODO b\n// FIXME c\n// HACK d\n";
        let counts = AnnotationScanner::count_by_kind(src);
        let todo_count = counts.iter().find(|(k, _)| *k == AnnotationKind::Todo).map(|(_, c)| *c);
        assert_eq!(todo_count, Some(2));
        let fixme_count = counts.iter().find(|(k, _)| *k == AnnotationKind::Fixme).map(|(_, c)| *c);
        assert_eq!(fixme_count, Some(1));
    }

    #[test]
    fn annotation_kind_display() {
        assert_eq!(AnnotationKind::Todo.to_string(), "TODO");
        assert_eq!(AnnotationKind::Fixme.tag(), "FIXME");
        assert_eq!(AnnotationKind::Note.tag(), "NOTE");
        assert_eq!(AnnotationKind::Xxx.tag(), "XXX");
    }

    // ── DocCommentGenerator tests ─────────────────────────────────────

    #[test]
    fn doc_comment_generator_basic_fn() {
        let sig = "pub fn compute(x: i32, y: i32) -> f64 {";
        let doc = DocCommentGenerator::generate_rust(sig);
        assert!(doc[0].contains("compute"));
        assert!(doc.iter().any(|l| l.contains("# Arguments")));
        assert!(doc.iter().any(|l| l.contains("`x`")));
        assert!(doc.iter().any(|l| l.contains("`y`")));
        assert!(doc.iter().any(|l| l.contains("# Returns")));
    }

    #[test]
    fn doc_comment_generator_no_params_no_return() {
        let sig = "fn do_stuff() {";
        let doc = DocCommentGenerator::generate_rust(sig);
        assert!(doc[0].contains("do_stuff"));
        assert!(!doc.iter().any(|l| l.contains("# Arguments")));
        assert!(!doc.iter().any(|l| l.contains("# Returns")));
    }

    #[test]
    fn doc_comment_generator_self_method() {
        let sig = "pub fn name(&self, label: &str) -> String {";
        let doc = DocCommentGenerator::generate_rust(sig);
        // &self should be excluded from arguments
        assert!(doc.iter().any(|l| l.contains("`label`")));
        assert!(!doc.iter().any(|l| l.contains("`self`") || l.contains("`&self`")));
        assert!(doc.iter().any(|l| l.contains("# Returns")));
    }

    // ── line/block comment conversion tests ───────────────────────────

    #[test]
    fn line_comments_to_block_basic() {
        let lines = vec!["// first line", "// second line"];
        let result = line_comments_to_block(&lines, "//", "/*", "*/");
        assert_eq!(result, vec!["/* first line second line */"]);
    }

    #[test]
    fn line_comments_to_block_mixed_with_code() {
        let lines = vec!["// comment", "let x = 1;", "// another"];
        let result = line_comments_to_block(&lines, "//", "/*", "*/");
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "/* comment */");
        assert_eq!(result[1], "let x = 1;");
        assert_eq!(result[2], "/* another */");
    }

    #[test]
    fn block_comment_to_lines_basic() {
        let text = "/* hello world */";
        let result = block_comment_to_lines(text, "//", "/*", "*/");
        assert_eq!(result, vec!["// hello world"]);
    }

    #[test]
    fn block_comment_to_lines_multiline() {
        let text = "/* line one\n * line two\n * line three */";
        let result = block_comment_to_lines(text, "//", "/*", "*/");
        assert_eq!(result, vec!["// line one", "// line two", "// line three"]);
    }

    #[test]
    fn block_comment_to_lines_not_a_block() {
        let text = "just plain text";
        let result = block_comment_to_lines(text, "//", "/*", "*/");
        assert_eq!(result, vec!["just plain text"]);
    }

    // ── indentation helper tests ──────────────────────────────────────

    #[test]
    fn min_indentation_basic() {
        let lines = vec!["    fn foo() {", "        bar();", "    }"];
        assert_eq!(min_indentation(&lines), 4);
    }

    #[test]
    fn min_indentation_with_blanks() {
        let lines = vec!["  a", "", "    b", "   "];
        assert_eq!(min_indentation(&lines), 2);
    }

    #[test]
    fn min_indentation_empty() {
        let lines: Vec<&str> = vec![];
        assert_eq!(min_indentation(&lines), 0);
    }

    #[test]
    fn reindent_comments_moves_to_target() {
        let lines = vec!["// hello", "  // world", "code"];
        let result = reindent_comments(&lines, "//", 4);
        assert_eq!(result[0], "    // hello");
        assert_eq!(result[1], "    // world");
        assert_eq!(result[2], "code");
    }

    #[test]
    fn dedent_comments_basic() {
        let lines = vec!["    // hello", "        // world", "code"];
        let result = dedent_comments(&lines, "//", 4);
        assert_eq!(result[0], "// hello");
        assert_eq!(result[1], "    // world");
        assert_eq!(result[2], "code");
    }

    #[test]
    fn dedent_comments_saturates_at_zero() {
        let lines = vec!["// already at col 0"];
        let result = dedent_comments(&lines, "//", 10);
        assert_eq!(result[0], "// already at col 0");
    }

    // ── Rustdoc parser tests ──────────────────────────────────────────

    #[test]
    fn parse_rustdoc_sections_basic() {
        let lines = vec![
            "/// A summary line.",
            "///",
            "/// # Arguments",
            "///",
            "/// * `x` - the input",
            "/// # Returns",
            "///",
            "/// The output value.",
        ];
        let sections = parse_rustdoc_sections(&lines);
        assert_eq!(sections.len(), 3);

        assert_eq!(sections[0].heading, None);
        assert!(sections[0].body.iter().any(|l| l.contains("summary")));

        assert_eq!(sections[1].heading.as_deref(), Some("Arguments"));
        assert!(sections[1].body.iter().any(|l| l.contains("`x`")));

        assert_eq!(sections[2].heading.as_deref(), Some("Returns"));
        assert!(sections[2].body.iter().any(|l| l.contains("output")));
    }

    #[test]
    fn parse_rustdoc_sections_no_headings() {
        let lines = vec!["/// Just a simple doc comment.", "/// Second line."];
        let sections = parse_rustdoc_sections(&lines);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].heading, None);
        assert_eq!(sections[0].body.len(), 2);
    }

    #[test]
    fn parse_rustdoc_ignores_non_doc_lines() {
        let lines = vec!["fn foo() {", "/// doc line", "let x = 1;"];
        let sections = parse_rustdoc_sections(&lines);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].body.len(), 1);
        assert!(sections[0].body[0].contains("doc line"));
    }

    // ── JSDoc parser tests ────────────────────────────────────────────

    #[test]
    fn parse_jsdoc_sections_basic() {
        let text = "/**\n * Computes a value.\n * @param x The input.\n * @returns The output.\n */";
        let sections = parse_jsdoc_sections(text);
        assert!(sections.len() >= 3);

        assert_eq!(sections[0].heading, None);
        assert!(sections[0].body.iter().any(|l| l.contains("Computes")));

        assert_eq!(sections[1].heading.as_deref(), Some("@param"));
        assert!(sections[1].body.iter().any(|l| l.contains("input")));

        assert_eq!(sections[2].heading.as_deref(), Some("@returns"));
    }

    #[test]
    fn parse_jsdoc_empty_block() {
        let text = "/** */";
        let sections = parse_jsdoc_sections(text);
        assert!(sections.is_empty());
    }

    // ── line classifier tests ─────────────────────────────────────────

    #[test]
    fn classify_line_blank() {
        assert_eq!(classify_line("", "//"), LineKind::Blank);
        assert_eq!(classify_line("   ", "//"), LineKind::Blank);
    }

    #[test]
    fn classify_line_full_comment() {
        assert_eq!(classify_line("// hello", "//"), LineKind::FullComment);
        assert_eq!(classify_line("  // indented", "//"), LineKind::FullComment);
    }

    #[test]
    fn classify_line_code_with_trailing() {
        assert_eq!(
            classify_line("let x = 1; // value", "//"),
            LineKind::CodeWithTrailingComment
        );
    }

    #[test]
    fn classify_line_pure_code() {
        assert_eq!(classify_line("let x = 1;", "//"), LineKind::Code);
    }

    #[test]
    fn classify_lines_returns_all() {
        let lines = vec!["// comment", "code", "", "x // trailing"];
        let classified = classify_lines(&lines, "//");
        assert_eq!(classified.len(), 4);
        assert_eq!(classified[0], (0, LineKind::FullComment));
        assert_eq!(classified[1], (1, LineKind::Code));
        assert_eq!(classified[2], (2, LineKind::Blank));
        assert_eq!(classified[3], (3, LineKind::CodeWithTrailingComment));
    }

    // ── strip trailing comments tests ─────────────────────────────────

    #[test]
    fn strip_trailing_comments_removes_inline() {
        let lines = vec!["let x = 1; // value", "// full comment", "code", ""];
        let result = strip_trailing_comments(&lines, "//");
        assert_eq!(result[0], "let x = 1;");
        assert_eq!(result[1], "// full comment");
        assert_eq!(result[2], "code");
        assert_eq!(result[3], "");
    }

    #[test]
    fn strip_trailing_comments_no_change_needed() {
        let lines = vec!["no comments here", "// full line comment"];
        let result = strip_trailing_comments(&lines, "//");
        assert_eq!(result[0], "no comments here");
        assert_eq!(result[1], "// full line comment");
    }

    // ── comment region builder tests ──────────────────────────────────

    #[test]
    fn build_comment_region_basic() {
        let result = build_comment_region("Section", "//", 40);
        assert_eq!(result.len(), 3);
        assert!(result[0].starts_with("// "));
        assert!(result[1].contains("Section"));
        assert!(result[1].starts_with("// "));
        assert!(result[2].starts_with("// "));
    }

    #[test]
    fn build_comment_region_narrow_fallback() {
        let result = build_comment_region("Very Long Title That Exceeds Width", "//", 10);
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("Very Long Title"));
    }

    #[test]
    fn build_comment_region_hash_prefix() {
        let result = build_comment_region("test", "#", 30);
        assert_eq!(result.len(), 3);
        assert!(result[0].starts_with("# "));
        assert!(result[1].contains("test"));
    }

#[test]
    fn commentblockformatter_severity_ordering() {
        assert!(CommentBlockFormatterSeverity::Critical > CommentBlockFormatterSeverity::High);
        assert!(CommentBlockFormatterSeverity::High > CommentBlockFormatterSeverity::Medium);
        assert!(CommentBlockFormatterSeverity::Medium > CommentBlockFormatterSeverity::Low);
    }

    #[test]
    fn commentblockformatter_severity_display() {
        assert_eq!(CommentBlockFormatterSeverity::Low.to_string(), "low");
        assert_eq!(CommentBlockFormatterSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn commentblockformatter_entry_creation() {
        let e = CommentBlockFormatterEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, CommentBlockFormatterSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn commentblockformatter_entry_builder() {
        let e = CommentBlockFormatterEntry::new("e2", "Entry 2")
            .with_severity(CommentBlockFormatterSeverity::High)
            .with_detail("some detail")
            .with_line_count(42);
        assert_eq!(e.severity, CommentBlockFormatterSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.line_count, 42);
    }

    #[test]
    fn commentblockformatter_entry_enable_disable() {
        let mut e = CommentBlockFormatterEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn commentblockformatter_add_and_count() {
        let mut mgr = CommentBlockFormatter::new("test");
        mgr.add(CommentBlockFormatterEntry::new("a", "A"));
        mgr.add(CommentBlockFormatterEntry::new("b", "B").with_severity(CommentBlockFormatterSeverity::High));
        assert_eq!(mgr.line_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn commentblockformatter_remove() {
        let mut mgr = CommentBlockFormatter::new("test");
        mgr.add(CommentBlockFormatterEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn commentblockformatter_capacity() {
        let mut mgr = CommentBlockFormatter::new("test").with_capacity(1);
        assert!(mgr.add(CommentBlockFormatterEntry::new("a", "A")));
        assert!(!mgr.add(CommentBlockFormatterEntry::new("b", "B")));
    }

    #[test]
    fn commentblockformatter_sorted_by_severity() {
        let mut mgr = CommentBlockFormatter::new("test");
        mgr.add(CommentBlockFormatterEntry::new("lo", "Low"));
        mgr.add(CommentBlockFormatterEntry::new("hi", "High").with_severity(CommentBlockFormatterSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, CommentBlockFormatterSeverity::Critical);
    }

    #[test]
    fn commentblockformatter_summary() {
        let mgr = CommentBlockFormatter::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn commenttoggle_config_defaults() {
        let cfg = CommentToggleConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn commenttoggle_item_creation() {
        let item = CommentToggleItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn commenttoggle_add_and_get() {
        let mut mgr = CommentToggle::new(CommentToggleConfig::new("test"));
        mgr.add(CommentToggleItem::new("k1", "v1"));
        assert_eq!(mgr.comment_style_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn commenttoggle_remove_item() {
        let mut mgr = CommentToggle::new(CommentToggleConfig::new("test"));
        mgr.add(CommentToggleItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn commenttoggle_sorted_by_priority() {
        let mut mgr = CommentToggle::new(CommentToggleConfig::new("test"));
        mgr.add(CommentToggleItem::new("lo", "low").with_priority(1));
        mgr.add(CommentToggleItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn commenttoggle_items_with_tag() {
        let mut mgr = CommentToggle::new(CommentToggleConfig::new("test"));
        mgr.add(CommentToggleItem::new("a", "1").with_tag("x"));
        mgr.add(CommentToggleItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn commenttoggle_report() {
        let mgr = CommentToggle::new(CommentToggleConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn comment_entry_creation() {
        let e = CommentEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn comment_entry_with_priority() {
        let e = CommentEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn comment_entry_metadata() {
        let e = CommentEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn comment_entry_remove_meta() {
        let mut e = CommentEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn comment_entry_activate_deactivate() {
        let mut e = CommentEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn comment_config_add_sorted() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("lo", "Lo").with_priority(1));
        c.add(CommentEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn comment_config_capacity() {
        let mut c = CommentConfig::new(1);
        assert!(c.add(CommentEntry::new("a", "A")));
        assert!(!c.add(CommentEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn comment_config_remove() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn comment_config_get() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn comment_config_active_entries() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("a", "A"));
        c.add(CommentEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn comment_config_enable_disable() {
        let mut c = CommentConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn comment_config_clear() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn comment_config_find_by_label() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn comment_config_top_n() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("a", "A").with_priority(1));
        c.add(CommentEntry::new("b", "B").with_priority(2));
        c.add(CommentEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn comment_config_deactivate_activate_all() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("a", "A"));
        c.add(CommentEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn comment_config_highest_priority() {
        let mut c = CommentConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(CommentEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn comment_config_contains() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn comment_config_labels() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("a", "Alpha"));
        c.add(CommentEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn comment_config_drain_inactive() {
        let mut c = CommentConfig::new(10);
        c.add(CommentEntry::new("a", "A"));
        c.add(CommentEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qo_metrics_empty() {
        let m = QoMetrics::new("comment");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qo_metrics_record_and_mean() {
        let mut m = QoMetrics::new("comment");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qo_metrics_min_max() {
        let mut m = QoMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qo_metrics_variance_and_std() {
        let mut m = QoMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qo_metrics_percentile() {
        let mut m = QoMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qo_metrics_merge() {
        let mut a = QoMetrics::new("a");
        a.record(1.0);
        let mut b = QoMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qo_metrics_reset() {
        let mut m = QoMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qo_rate_window_empty() {
        let rw = QoRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qo_rate_window_tick_and_rate() {
        let mut rw = QoRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qo_lru_cache_basic() {
        let mut c = QoLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qo_lru_cache_contains_and_keys() {
        let mut c = QoLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qo_lru_cache_remove() {
        let mut c = QoLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qo_metrics_sum() {
        let mut m = QoMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qo_metrics_label() {
        let m = QoMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qo_lru_cache_clear() {
        let mut c = QoLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_2_push_and_len() {
        let mut rb = super::XbRingBuffer2::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_2_overwrite() {
        let mut rb = super::XbRingBuffer2::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_2_get_out_of_bounds() {
        let rb = super::XbRingBuffer2::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_2_drain_all() {
        let mut rb = super::XbRingBuffer2::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_2_peek_front_back() {
        let mut rb = super::XbRingBuffer2::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_2_clear() {
        let mut rb = super::XbRingBuffer2::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_2_capacity() {
        let rb = super::XbRingBuffer2::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_2_basic() {
        let h = super::xb_fnv1a_2(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_2(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_2_different_inputs() {
        let h1 = super::xb_fnv1a_2(b"abc");
        let h2 = super::xb_fnv1a_2(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_2_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_2(&data);
        let dec = super::xb_rle_decode_2(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_2_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_2(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_2(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_2_values() {
        assert!((super::xb_clamp_2(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_2(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_2(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_2_values() {
        assert!((super::xb_lerp_2(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_2(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_2(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_2_wrap_around_twice() {
        let mut rb = super::XbRingBuffer2::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 20 ----

    #[test]
    fn xc_20_pool_new_empty() {
        let pool: super::Xc20Pool<i32> = super::Xc20Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_20_pool_release_acquire() {
        let mut pool = super::Xc20Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_20_pool_acquire_empty() {
        let mut pool: super::Xc20Pool<i32> = super::Xc20Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_20_pool_full() {
        let mut pool = super::Xc20Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_20_pool_drain() {
        let mut pool = super::Xc20Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_20_pool_stats() {
        let mut pool = super::Xc20Pool::new(8);
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
    fn xc_20_pool_clear() {
        let mut pool = super::Xc20Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_20_pool_shrink() {
        let mut pool = super::Xc20Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_20_pool_default() {
        let pool: super::Xc20Pool<String> = super::Xc20Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_20_pool_extend() {
        let mut pool = super::Xc20Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_20_pool_retain() {
        let mut pool = super::Xc20Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_20_scheduler_round_robin() {
        let mut sched = super::Xc20Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_20_scheduler_empty() {
        let mut sched = super::Xc20Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_20_scheduler_reset() {
        let mut sched = super::Xc20Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_20_scheduler_add_remove() {
        let mut sched = super::Xc20Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_20_scheduler_targets() {
        let sched = super::Xc20Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_20_hash_empty() {
        assert_eq!(super::xc_20_hash(b""), 5381);
    }

    #[test]
    fn xc_20_hash_data() {
        let h = super::xc_20_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_20_hash(b"hello"), h);
    }

    #[test]
    fn xc_20_reverse_str() {
        assert_eq!(super::xc_20_reverse("abc"), "cba");
        assert_eq!(super::xc_20_reverse(""), "");
    }


    #[test]
    fn xe_2_pipeline_empty() {
        let p = super::Xe2Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_2_pipeline_parse_stage() {
        let p = super::Xe2Pipeline::new()
            .add_parse(super::xe_2_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_2_pipeline_transform_double() {
        let p = super::Xe2Pipeline::new()
            .add_transform(super::xe_2_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_2_pipeline_validate_reverse() {
        let p = super::Xe2Pipeline::new()
            .add_validate(super::xe_2_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_2_pipeline_emit_filter() {
        let p = super::Xe2Pipeline::new()
            .add_emit(super::xe_2_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_2_pipeline_multi_stage() {
        let p = super::Xe2Pipeline::new()
            .add_parse(super::xe_2_pipeline_identity)
            .add_transform(super::xe_2_pipeline_double)
            .add_validate(super::xe_2_pipeline_reverse)
            .add_emit(super::xe_2_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_2_pipeline_error_propagation() {
        let p = super::Xe2Pipeline::new()
            .add_parse(super::xe_2_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe2Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_2_pipeline_compose() {
        let p1 = super::Xe2Pipeline::new()
            .add_parse(super::xe_2_pipeline_identity);
        let p2 = super::Xe2Pipeline::new()
            .add_transform(super::xe_2_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_2_pipeline_error_display() {
        let e = super::Xe2PipelineError {
            stage: super::Xe2Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_2_cache_put_get() {
        let mut c = super::Xe2Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_2_cache_miss() {
        let mut c: super::Xe2Cache<&str, i32> = super::Xe2Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_2_cache_ttl_expiry() {
        let mut c = super::Xe2Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_2_cache_evict() {
        let mut c = super::Xe2Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_2_cache_capacity() {
        let mut c = super::Xe2Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_2_cache_stats() {
        let mut c = super::Xe2Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_2_cache_clear() {
        let mut c = super::Xe2Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #63 --

    #[test]
    fn xf63_trie_insert_search() {
        let mut t = Xf63Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf63_trie_starts_with() {
        let mut t = Xf63Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf63_trie_remove() {
        let mut t = Xf63Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf63_trie_word_count() {
        let mut t = Xf63Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf63_trie_longest_prefix() {
        let mut t = Xf63Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf63_trie_all_words() {
        let mut t = Xf63Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf63_trie_autocomplete() {
        let mut t = Xf63Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf63_trie_empty_search() {
        let t = Xf63Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf63_bloom_add_contains() {
        let mut bf = Xf63BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf63_bloom_probably_absent() {
        let bf = Xf63BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf63_bloom_false_positive_rate() {
        let mut bf = Xf63BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf63_bloom_clear() {
        let mut bf = Xf63BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf63_bloom_union() {
        let mut a = Xf63BloomFilter::xf_new(512, 2);
        let mut b = Xf63BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf63_bloom_intersection_estimate() {
        let mut a = Xf63BloomFilter::xf_new(512, 2);
        let mut b = Xf63BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf63_bloom_union_size_mismatch() {
        let a = Xf63BloomFilter::xf_new(256, 2);
        let b = Xf63BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh19_skip_insert_contains() {
        let mut sl = super::Xh19SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh19_skip_remove() {
        let mut sl = super::Xh19SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh19_skip_len() {
        let mut sl = super::Xh19SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh19_skip_range_query() {
        let mut sl = super::Xh19SkipList::xh_new(4);
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
    fn xh19_skip_floor_ceiling() {
        let mut sl = super::Xh19SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh19_skip_rank() {
        let mut sl = super::Xh19SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh19_skip_empty() {
        let sl = super::Xh19SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh19_skip_duplicates() {
        let mut sl = super::Xh19SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh19_bitset_set_test() {
        let mut bs = super::Xh19BitSet::xh_new(256);
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
    fn xh19_bitset_clear_count() {
        let mut bs = super::Xh19BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh19_bitset_and_or_xor() {
        let mut a = super::Xh19BitSet::xh_new(128);
        let mut b = super::Xh19BitSet::xh_new(128);
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
    fn xh19_bitset_iter_ones() {
        let mut bs = super::Xh19BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh19_bitset_first_last() {
        let mut bs = super::Xh19BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh19_bitset_empty() {
        let bs = super::Xh19BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi19_deque_push_pop_back() {
        let mut dq = super::Xi19Deque::xi_new(4);
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
    fn xi19_deque_push_pop_front() {
        let mut dq = super::Xi19Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi19_deque_mixed_ops() {
        let mut dq = super::Xi19Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi19_deque_get_and_split() {
        let mut dq = super::Xi19Deque::xi_new(8);
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
    fn xi19_deque_rotate_left() {
        let mut dq = super::Xi19Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi19_deque_rotate_right() {
        let mut dq = super::Xi19Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi19_deque_grow() {
        let mut dq = super::Xi19Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi19_deque_empty() {
        let dq = super::Xi19Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi19_interval_tree_insert_query() {
        let mut tree = super::Xi19IntervalTree::xi_new();
        tree.xi_insert(super::Xi19Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi19Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi19Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi19_interval_tree_overlap() {
        let mut tree = super::Xi19IntervalTree::xi_new();
        tree.xi_insert(super::Xi19Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi19Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi19Interval::xi_new(12, 20));
        let q = super::Xi19Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi19_interval_tree_remove() {
        let mut tree = super::Xi19IntervalTree::xi_new();
        tree.xi_insert(super::Xi19Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi19Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi19_interval_tree_gaps() {
        let mut tree = super::Xi19IntervalTree::xi_new();
        tree.xi_insert(super::Xi19Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi19Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi19Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi19Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi19Interval::xi_new(8, 10));
    }

    #[test]
    fn xi19_interval_tree_merge() {
        let mut tree = super::Xi19IntervalTree::xi_new();
        tree.xi_insert(super::Xi19Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi19Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi19Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi19Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi19Interval::xi_new(10, 15));
    }

    #[test]
    fn xi19_interval_tree_all() {
        let mut tree = super::Xi19IntervalTree::xi_new();
        tree.xi_insert(super::Xi19Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi19Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi19_interval_tree_empty() {
        let tree = super::Xi19IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi19_interval_tree_contains_point() {
        let iv = super::Xi19Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
