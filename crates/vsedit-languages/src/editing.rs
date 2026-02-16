//! Editing helpers: comment toggling, auto-closing, word patterns.

use regex::Regex;

use crate::config::LanguageEditConfig;
use crate::registry::LanguageService;

// ---------------------------------------------------------------------------
// Comment toggling
// ---------------------------------------------------------------------------

/// Toggle a line comment on a single line of text.
///
/// When `remove` is true the comment prefix is stripped; otherwise it is added.
pub fn toggle_line_comment(prefix: &str, text: &str, remove: bool) -> String {
    if remove {
        let trimmed = text.trim_start();
        if trimmed.starts_with(prefix) {
            let after = &trimmed[prefix.len()..];
            // Strip the optional single space after the prefix.
            let after = after.strip_prefix(' ').unwrap_or(after);
            let leading: &str = &text[..text.len() - trimmed.len()];
            format!("{leading}{after}")
        } else {
            text.to_string()
        }
    } else {
        format!("{prefix} {text}")
    }
}

/// Toggle a block comment around `text`.
///
/// When `remove` is true the block delimiters are stripped; otherwise they are
/// added.
pub fn toggle_block_comment(open: &str, close: &str, text: &str, remove: bool) -> String {
    if remove {
        let t = text.trim();
        if t.starts_with(open) && t.ends_with(close) {
            let inner = &t[open.len()..t.len() - close.len()];
            inner.trim().to_string()
        } else {
            text.to_string()
        }
    } else {
        format!("{open} {text} {close}")
    }
}

// ---------------------------------------------------------------------------
// Auto-closing
// ---------------------------------------------------------------------------

/// Check whether `open_char` should auto-close in the given `context`.
///
/// Returns `false` when the pair's `not_in` list includes the context.
pub fn should_auto_close(cfg: &LanguageEditConfig, open_char: &str, context: &str) -> bool {
    // Find matching auto-closing pair.
    for pair in &cfg.auto_closing_pairs {
        if pair.open == open_char {
            return !pair.not_in.iter().any(|ctx| ctx == context);
        }
    }
    false
}

/// Get the closing string for `open_char` from auto-closing pairs.
pub fn get_matching_bracket(cfg: &LanguageEditConfig, open_char: &str) -> Option<String> {
    cfg.auto_closing_pairs
        .iter()
        .find(|p| p.open == open_char)
        .map(|p| p.close.clone())
}

// ---------------------------------------------------------------------------
// Word pattern
// ---------------------------------------------------------------------------

const DEFAULT_WORD_PATTERN: &str = r"[a-zA-Z_]\w*";

/// Return the word-boundary regex for a language.
///
/// Falls back to the default `[a-zA-Z_]\w*` when no language-specific pattern
/// is configured.
pub fn get_word_pattern(service: &LanguageService, language_id: &str) -> Regex {
    if let Some(cfg) = service.get_edit_config(language_id) {
        if let Some(ref re) = cfg.word_pattern {
            return re.clone();
        }
    }
    Regex::new(DEFAULT_WORD_PATTERN).unwrap()
}
