//! Emmet abbreviation expansion.

/// Controls when expanded abbreviations are shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShowExpanded {
    Always,
    Never,
    InMarkupAndStylesheetFilesOnly,
}

/// Configuration for Emmet expansion behavior.
#[derive(Debug, Clone)]
pub struct EmmetConfig {
    pub enabled: bool,
    pub show_abbreviation_suggestions: bool,
    pub show_expanded_abbreviation: ShowExpanded,
    pub syntaxes: Vec<String>,
}

impl Default for EmmetConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_abbreviation_suggestions: true,
            show_expanded_abbreviation: ShowExpanded::Always,
            syntaxes: vec![
                "html".to_string(),
                "css".to_string(),
                "xml".to_string(),
            ],
        }
    }
}

/// Returns `true` if `input` looks like a valid Emmet abbreviation.
pub fn is_abbreviation(input: &str) -> bool {
    !input.is_empty()
        && input
            .chars()
            .all(|c| c.is_alphanumeric() || ".#>{}+*".contains(c))
}

/// Expand a basic Emmet abbreviation into HTML.
///
/// Only a small subset of Emmet syntax is supported:
/// - `tag`           → `<tag></tag>`
/// - `tag.class`     → `<tag class="class"></tag>`
/// - `tag#id`        → `<tag id="id"></tag>`
/// - `parent>child`  → nested elements
/// - `tag{text}`     → `<tag>text</tag>`
///
/// Returns `None` for unsupported or invalid abbreviations.
pub fn expand_abbreviation(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() || !is_abbreviation(input) {
        return None;
    }

    // parent>child
    if let Some(pos) = input.find('>') {
        let parent = &input[..pos];
        let child = &input[pos + 1..];
        let child_expanded = expand_abbreviation(child)?;
        let indented: String = child_expanded
            .lines()
            .map(|l| format!("  {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        return Some(format!("<{parent}>\n{indented}\n</{parent}>"));
    }

    // tag{text}
    if let Some(brace) = input.find('{') {
        if input.ends_with('}') {
            let tag = &input[..brace];
            let text = &input[brace + 1..input.len() - 1];
            if tag.is_empty() {
                return None;
            }
            return Some(format!("<{tag}>{text}</{tag}>"));
        }
        return None;
    }

    // tag.class
    if let Some(dot) = input.find('.') {
        let tag = &input[..dot];
        let class = &input[dot + 1..];
        if tag.is_empty() || class.is_empty() {
            return None;
        }
        return Some(format!("<{tag} class=\"{class}\"></{tag}>"));
    }

    // tag#id
    if let Some(hash) = input.find('#') {
        let tag = &input[..hash];
        let id = &input[hash + 1..];
        if tag.is_empty() || id.is_empty() {
            return None;
        }
        return Some(format!("<{tag} id=\"{id}\"></{tag}>"));
    }

    // plain tag
    if input.chars().all(|c| c.is_alphanumeric()) {
        return Some(format!("<{input}></{input}>"));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_plain_tag() {
        assert_eq!(expand_abbreviation("div"), Some("<div></div>".to_string()));
    }

    #[test]
    fn expand_class() {
        assert_eq!(
            expand_abbreviation("div.class"),
            Some("<div class=\"class\"></div>".to_string()),
        );
    }

    #[test]
    fn expand_id() {
        assert_eq!(
            expand_abbreviation("div#id"),
            Some("<div id=\"id\"></div>".to_string()),
        );
    }

    #[test]
    fn expand_child() {
        assert_eq!(
            expand_abbreviation("ul>li"),
            Some("<ul>\n  <li></li>\n</ul>".to_string()),
        );
    }

    #[test]
    fn expand_text_content() {
        assert_eq!(
            expand_abbreviation("p{text}"),
            Some("<p>text</p>".to_string()),
        );
    }

    #[test]
    fn unknown_returns_none() {
        assert_eq!(expand_abbreviation(""), None);
        assert_eq!(expand_abbreviation("  "), None);
    }

    #[test]
    fn is_abbreviation_basic() {
        assert!(is_abbreviation("div.foo"));
        assert!(is_abbreviation("ul>li"));
        assert!(!is_abbreviation("hello world"));
        assert!(!is_abbreviation(""));
    }

    #[test]
    fn default_config() {
        let cfg = EmmetConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.show_expanded_abbreviation, ShowExpanded::Always);
    }
}
