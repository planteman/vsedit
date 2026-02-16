//! Per-language editing configuration: comments, brackets, auto-closing,
//! folding markers, indentation rules, word patterns, and on-enter rules.

use regex::Regex;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Public configuration types
// ---------------------------------------------------------------------------

/// Per-language editing behaviour.
#[derive(Debug, Clone)]
pub struct LanguageEditConfig {
    /// Comment configuration.
    pub comments: CommentConfig,
    /// Bracket pairs for matching / highlighting.
    pub brackets: Vec<BracketPair>,
    /// Pairs that auto-close when the opening character is typed.
    pub auto_closing_pairs: Vec<AutoClosingPair>,
    /// Pairs used when surrounding selected text.
    pub surrounding_pairs: Vec<BracketPair>,
    /// Optional folding marker regexes.
    pub folding_markers: Option<FoldingMarkers>,
    /// Optional indentation rules.
    pub indentation_rules: Option<IndentationRules>,
    /// Optional language-specific word pattern.
    pub word_pattern: Option<Regex>,
    /// On-enter rules for automatic indentation.
    pub on_enter_rules: Vec<OnEnterRule>,
}

/// Comment configuration for a language.
#[derive(Debug, Clone, Default)]
pub struct CommentConfig {
    /// Line comment prefix (e.g. `"//"`).
    pub line_comment: Option<String>,
    /// Block comment delimiters (e.g. `("/*", "*/")`).
    pub block_comment: Option<(String, String)>,
}

/// A bracket pair.
#[derive(Debug, Clone)]
pub struct BracketPair {
    pub open: String,
    pub close: String,
}

/// Auto-closing pair with optional exclusion contexts.
#[derive(Debug, Clone)]
pub struct AutoClosingPair {
    pub open: String,
    pub close: String,
    /// Contexts where auto-closing is suppressed (e.g. `["string", "comment"]`).
    pub not_in: Vec<String>,
}

/// Start/end regex markers for code folding regions.
#[derive(Debug, Clone)]
pub struct FoldingMarkers {
    pub start: Regex,
    pub end: Regex,
}

/// Regex-based indentation rules.
#[derive(Debug, Clone)]
pub struct IndentationRules {
    pub increase_indent_pattern: Regex,
    pub decrease_indent_pattern: Regex,
    pub indent_next_line_pattern: Option<Regex>,
    pub unindented_line_pattern: Option<Regex>,
}

/// Indent action applied by on-enter rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndentAction {
    None,
    Indent,
    IndentOutdent,
    Outdent,
}

/// Rule applied when Enter is pressed.
#[derive(Debug, Clone)]
pub struct OnEnterRule {
    pub before_text: Regex,
    pub after_text: Option<Regex>,
    pub action: IndentAction,
}

// ---------------------------------------------------------------------------
// Default — common bracket / pair configuration shared by most languages
// ---------------------------------------------------------------------------

impl Default for LanguageEditConfig {
    fn default() -> Self {
        let brackets = vec![
            BracketPair { open: "(".into(), close: ")".into() },
            BracketPair { open: "[".into(), close: "]".into() },
            BracketPair { open: "{".into(), close: "}".into() },
        ];

        let auto_closing_pairs = vec![
            AutoClosingPair { open: "(".into(), close: ")".into(), not_in: vec![] },
            AutoClosingPair { open: "[".into(), close: "]".into(), not_in: vec![] },
            AutoClosingPair { open: "{".into(), close: "}".into(), not_in: vec![] },
            AutoClosingPair {
                open: "\"".into(),
                close: "\"".into(),
                not_in: vec!["string".into()],
            },
            AutoClosingPair {
                open: "'".into(),
                close: "'".into(),
                not_in: vec!["string".into(), "comment".into()],
            },
        ];

        let surrounding_pairs = vec![
            BracketPair { open: "(".into(), close: ")".into() },
            BracketPair { open: "[".into(), close: "]".into() },
            BracketPair { open: "{".into(), close: "}".into() },
            BracketPair { open: "\"".into(), close: "\"".into() },
            BracketPair { open: "'".into(), close: "'".into() },
            BracketPair { open: "`".into(), close: "`".into() },
        ];

        Self {
            comments: CommentConfig::default(),
            brackets,
            auto_closing_pairs,
            surrounding_pairs,
            folding_markers: None,
            indentation_rules: None,
            word_pattern: None,
            on_enter_rules: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// JSON parsing — language-configuration.json files from VS Code extensions
// ---------------------------------------------------------------------------

/// Parse a VS Code `language-configuration.json` string into a
/// [`LanguageEditConfig`].
pub fn parse_language_configuration(json: &str) -> Result<LanguageEditConfig, String> {
    let raw: RawLangConfig =
        serde_json::from_str(json).map_err(|e| format!("invalid json: {e}"))?;

    let comments = match raw.comments {
        Some(c) => CommentConfig {
            line_comment: c.line_comment,
            block_comment: c.block_comment.map(|v| (v[0].clone(), v[1].clone())),
        },
        None => CommentConfig::default(),
    };

    let brackets = raw
        .brackets
        .unwrap_or_default()
        .into_iter()
        .map(|v| BracketPair { open: v[0].clone(), close: v[1].clone() })
        .collect();

    let auto_closing_pairs = raw
        .auto_closing_pairs
        .unwrap_or_default()
        .into_iter()
        .map(|p| AutoClosingPair {
            open: p.open,
            close: p.close,
            not_in: p.not_in.unwrap_or_default(),
        })
        .collect();

    let folding_markers = raw.folding.and_then(|f| {
        f.markers.and_then(|m| {
            let start = Regex::new(&m.start).ok()?;
            let end = Regex::new(&m.end).ok()?;
            Some(FoldingMarkers { start, end })
        })
    });

    let indentation_rules = raw.indentation_rules.and_then(|ir| {
        let inc = Regex::new(&ir.increase_indent_pattern).ok()?;
        let dec = Regex::new(&ir.decrease_indent_pattern).ok()?;
        let next = ir.indent_next_line_pattern.and_then(|p| Regex::new(&p).ok());
        let unind = ir.unindented_line_pattern.and_then(|p| Regex::new(&p).ok());
        Some(IndentationRules {
            increase_indent_pattern: inc,
            decrease_indent_pattern: dec,
            indent_next_line_pattern: next,
            unindented_line_pattern: unind,
        })
    });

    let word_pattern = raw.word_pattern.and_then(|p| Regex::new(&p).ok());

    Ok(LanguageEditConfig {
        comments,
        brackets,
        auto_closing_pairs,
        surrounding_pairs: Vec::new(),
        folding_markers,
        indentation_rules,
        word_pattern,
        on_enter_rules: Vec::new(),
    })
}

// -- serde helper structs ---------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLangConfig {
    comments: Option<RawComments>,
    brackets: Option<Vec<Vec<String>>>,
    auto_closing_pairs: Option<Vec<RawAutoClosingPair>>,
    folding: Option<RawFolding>,
    indentation_rules: Option<RawIndentationRules>,
    word_pattern: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawComments {
    line_comment: Option<String>,
    block_comment: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAutoClosingPair {
    open: String,
    close: String,
    not_in: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RawFolding {
    markers: Option<RawFoldingMarkers>,
}

#[derive(Deserialize)]
struct RawFoldingMarkers {
    start: String,
    end: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawIndentationRules {
    increase_indent_pattern: String,
    decrease_indent_pattern: String,
    indent_next_line_pattern: Option<String>,
    unindented_line_pattern: Option<String>,
}
