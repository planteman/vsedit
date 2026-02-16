//! VS Code–style line and block comment toggling.
//!
//! Provides enums and helpers to add, remove, or toggle line and block
//! comments for any language whose comment syntax is described by a
//! [`CommentRule`].

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

    /// Reset the detector to the initial (outside) state.
    pub fn reset(&mut self) {
        self.depth = 0;
    }
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
}
