//! Emmet abbreviation expansion.

use std::fmt;
/// Controls when expanded abbreviations are shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShowExpanded {
    Always,
    Never,
    InMarkupAndStylesheetFilesOnly,
}

impl ShowExpanded {
    /// Returns `true` if this variant is `Always`.
    pub fn is_always(&self) -> bool {
        matches!(self, ShowExpanded::Always)
    }
}

/// Emmet actions that can be triggered by the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmmetAction {
    Expand,
    Wrap,
    Balance,
    GoToMatching,
}

impl EmmetAction {
    /// Returns a human-readable label for the action.
    pub fn label(&self) -> &'static str {
        match self {
            EmmetAction::Expand => "Expand Abbreviation",
            EmmetAction::Wrap => "Wrap with Abbreviation",
            EmmetAction::Balance => "Balance (Select Matching)",
            EmmetAction::GoToMatching => "Go to Matching Pair",
        }
    }

    /// Returns `true` if this action is `Expand`.
    pub fn is_expand(&self) -> bool {
        matches!(self, EmmetAction::Expand)
    }
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

impl EmmetConfig {
    /// Returns `true` if the given syntax is in the supported list.
    pub fn is_syntax_supported(&self, syntax: &str) -> bool {
        self.syntaxes.iter().any(|s| s == syntax)
    }

    /// Adds a syntax to the supported list if not already present.
    pub fn add_syntax(&mut self, syntax: &str) {
        if !self.is_syntax_supported(syntax) {
            self.syntaxes.push(syntax.to_string());
        }
    }

    /// Removes a syntax from the supported list.
    pub fn remove_syntax(&mut self, syntax: &str) {
        self.syntaxes.retain(|s| s != syntax);
    }

    /// Returns true if syntaxes is empty.
    pub fn is_syntaxes_empty(&self) -> bool {
        self.syntaxes.is_empty()
    }

    /// Get the first syntaxe, if any.
    pub fn first_syntaxe(&self) -> Option<&String> {
        self.syntaxes.first()
    }

    /// Get the last syntaxe, if any.
    pub fn last_syntaxe(&self) -> Option<&String> {
        self.syntaxes.last()
    }

    /// Retain only syntaxes matching the predicate.
    pub fn retain_syntaxes(&mut self, f: impl Fn(&String) -> bool) {
        self.syntaxes.retain(|item| f(item));
    }

    /// Toggle the `show_abbreviation_suggestions` flag.
    pub fn toggle_show_abbreviation_suggestions(&mut self) {
        self.show_abbreviation_suggestions = !self.show_abbreviation_suggestions;
    }

    /// Returns the number of syntaxes in the supported list.
    pub fn syntax_count(&self) -> usize {
        self.syntaxes.len()
    }

    /// Returns `true` if abbreviation suggestions are enabled.
    pub fn has_suggestions_enabled(&self) -> bool {
        self.show_abbreviation_suggestions
    }
}

/// Returns the list of known self-closing HTML tags.
pub fn self_closing_tags() -> &'static [&'static str] {
    &["img", "br", "hr", "input", "meta", "link"]
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
/// Supported syntax:
/// - `tag`           → `<tag></tag>`
/// - `tag.class`     → `<tag class="class"></tag>`
/// - `tag#id`        → `<tag id="id"></tag>`
/// - `parent>child`  → nested elements
/// - `tag{text}`     → `<tag>text</tag>`
/// - `tag*N`         → repeat tag N times
/// - `tag+tag`       → sibling elements
///
/// Returns `None` for unsupported or invalid abbreviations.
pub fn expand_abbreviation(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() || !is_abbreviation(input) {
        return None;
    }

    // sibling: tag+tag
    if let Some(pos) = input.find('+') {
        let left = &input[..pos];
        let right = &input[pos + 1..];
        let left_expanded = expand_abbreviation(left)?;
        let right_expanded = expand_abbreviation(right)?;
        return Some(format!("{left_expanded}\n{right_expanded}"));
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

    // multiplication: tag*N
    if let Some(pos) = input.find('*') {
        let tag_part = &input[..pos];
        let count_str = &input[pos + 1..];
        let count: usize = count_str.parse().ok()?;
        if count == 0 {
            return None;
        }
        let single = expand_abbreviation(tag_part)?;
        let parts: Vec<&str> = std::iter::repeat(single.as_str()).take(count).collect();
        return Some(parts.join("\n"));
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

/// Expand a CSS Emmet abbreviation into a CSS property declaration.
///
/// Supported patterns:
/// - `m10`  → `margin: 10px;`
/// - `p10`  → `padding: 10px;`
/// - `w100` → `width: 100px;`
/// - `h50`  → `height: 50px;`
/// - `bgc`  → `background-color: ;`
pub fn expand_css_abbreviation(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    // keyword-only abbreviations
    match input {
        "bgc" => return Some("background-color: ;".to_string()),
        "ff" => return Some("font-family: ;".to_string()),
        "fs" => return Some("font-size: ;".to_string()),
        "fw" => return Some("font-weight: ;".to_string()),
        _ => {}
    }

    // property + numeric value patterns
    let prefixes: &[(&str, &str)] = &[
        ("m", "margin"),
        ("p", "padding"),
        ("w", "width"),
        ("h", "height"),
        ("t", "top"),
        ("b", "bottom"),
        ("l", "left"),
        ("r", "right"),
    ];

    for (abbr, prop) in prefixes {
        if let Some(rest) = input.strip_prefix(abbr) {
            if let Ok(val) = rest.parse::<i32>() {
                return Some(format!("{prop}: {val}px;"));
            }
        }
    }

    None
}

/// Wraps `content` inside the tag produced by expanding `abbreviation`.
pub fn get_wrap_abbreviation(content: &str, abbreviation: &str) -> Option<String> {
    let expanded = expand_abbreviation(abbreviation)?;
    if let Some(close_idx) = expanded.rfind("</") {
        let mut result = String::with_capacity(expanded.len() + content.len());
        result.push_str(&expanded[..close_idx]);
        result.push_str(content);
        result.push_str(&expanded[close_idx..]);
        Some(result)
    } else {
        None
    }
}

/// Like [`expand_abbreviation`] but first validates the syntax against the
/// provided [`EmmetConfig`].
pub fn expand_abbreviation_with_config(
    input: &str,
    config: &EmmetConfig,
) -> Option<String> {
    if !config.enabled {
        return None;
    }
    if !config.syntaxes.is_empty()
        && !config.is_syntax_supported("html")
        && !config.is_syntax_supported("xml")
    {
        return None;
    }
    expand_abbreviation(input)
}

/// Accumulated statistics for emmet operations.
#[derive(Debug, Clone, PartialEq)]
pub struct EmmetStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl EmmetStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &EmmetStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for EmmetStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EmmetStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EmmetStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for emmet.
#[derive(Debug, Clone)]
pub struct EmmetValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl EmmetValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for EmmetValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AbbreviationNode & EmmetAbbreviationParser
// ---------------------------------------------------------------------------

/// Parsed representation of an Emmet abbreviation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbbreviationNode {
    /// A single HTML tag with optional children.
    Tag {
        name: String,
        children: Vec<AbbreviationNode>,
    },
    /// A node repeated N times.
    Repeat {
        node: Box<AbbreviationNode>,
        count: usize,
    },
    /// Sibling nodes rendered sequentially.
    Sibling {
        nodes: Vec<AbbreviationNode>,
    },
}

impl AbbreviationNode {
    /// Render the node tree into HTML output.
    pub fn render(&self) -> String {
        match self {
            AbbreviationNode::Tag { name, children } => {
                if children.is_empty() {
                    format!("<{name}></{name}>")
                } else {
                    let inner: Vec<String> = children.iter().map(|c| c.render()).collect();
                    let inner_str = inner.join("\n");
                    let indented: String = inner_str
                        .lines()
                        .map(|l| format!("  {l}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("<{name}>\n{indented}\n</{name}>")
                }
            }
            AbbreviationNode::Repeat { node, count } => {
                let single = node.render();
                let parts: Vec<&str> = std::iter::repeat(single.as_str()).take(*count).collect();
                parts.join("\n")
            }
            AbbreviationNode::Sibling { nodes } => {
                let rendered: Vec<String> = nodes.iter().map(|n| n.render()).collect();
                rendered.join("\n")
            }
        }
    }
}

/// Parser for Emmet abbreviation syntax.
#[derive(Debug)]
pub struct EmmetAbbreviationParser {
    input: String,
}

impl EmmetAbbreviationParser {
    /// Create a new parser with the given input abbreviation.
    pub fn new(input: &str) -> Self {
        Self {
            input: input.trim().to_string(),
        }
    }

    /// Check if the input is a valid abbreviation.
    pub fn is_valid(&self) -> bool {
        is_abbreviation(&self.input)
    }

    /// Parse the input into an `AbbreviationNode` tree.
    pub fn parse(&self) -> Option<AbbreviationNode> {
        if !self.is_valid() {
            return None;
        }
        Self::parse_expr(&self.input)
    }

    fn parse_expr(input: &str) -> Option<AbbreviationNode> {
        if input.is_empty() {
            return None;
        }

        // Sibling: split on '+'
        if let Some(pos) = input.find('+') {
            let left = &input[..pos];
            let right = &input[pos + 1..];
            let left_node = Self::parse_expr(left)?;
            let right_node = Self::parse_expr(right)?;
            let mut nodes = Vec::new();
            // Flatten nested siblings
            match left_node {
                AbbreviationNode::Sibling { nodes: mut ln } => nodes.append(&mut ln),
                other => nodes.push(other),
            }
            match right_node {
                AbbreviationNode::Sibling { nodes: mut rn } => nodes.append(&mut rn),
                other => nodes.push(other),
            }
            return Some(AbbreviationNode::Sibling { nodes });
        }

        // Parent>child
        if let Some(pos) = input.find('>') {
            let parent = &input[..pos];
            let child = &input[pos + 1..];
            let child_node = Self::parse_expr(child)?;
            return Some(AbbreviationNode::Tag {
                name: parent.to_string(),
                children: vec![child_node],
            });
        }

        // Repeat: tag*N
        if let Some(pos) = input.find('*') {
            let tag_part = &input[..pos];
            let count_str = &input[pos + 1..];
            let count: usize = count_str.parse().ok()?;
            if count == 0 {
                return None;
            }
            let node = Self::parse_expr(tag_part)?;
            return Some(AbbreviationNode::Repeat {
                node: Box::new(node),
                count,
            });
        }

        // Plain tag name
        if input.chars().all(|c| c.is_alphanumeric()) {
            return Some(AbbreviationNode::Tag {
                name: input.to_string(),
                children: Vec::new(),
            });
        }

        None
    }
}

// ---------------------------------------------------------------------------
// Tag completion helpers
// ---------------------------------------------------------------------------

/// Given a partial opening tag like `<div`, returns the closing `</div>`.
pub fn tag_completion(partial: &str) -> Option<String> {
    let trimmed = partial.trim();
    let tag_content = trimmed.strip_prefix('<')?;
    if tag_content.is_empty() || tag_content.starts_with('/') {
        return None;
    }
    // Extract the tag name (first word)
    let tag_name = tag_content
        .split(|c: char| c.is_whitespace() || c == '>')
        .next()?;
    if tag_name.is_empty() {
        return None;
    }
    if !needs_closing_tag(tag_name) {
        return None;
    }
    Some(close_tag(tag_name))
}

/// Returns `false` for self-closing/void HTML tags.
pub fn needs_closing_tag(tag: &str) -> bool {
    !self_closing_tags().contains(&tag)
}

/// Returns a closing tag string like `</tag_name>`.
pub fn close_tag(tag_name: &str) -> String {
    format!("</{tag_name}>")
}

// ---------------------------------------------------------------------------
// Wrap with abbreviation
// ---------------------------------------------------------------------------

/// Wrap each selection with the abbreviation expansion, placing the selection
/// text inside the innermost tag.
pub fn emmet_wrap_with_abbreviation(selections: &[&str], abbreviation: &str) -> Option<Vec<String>> {
    // Validate the abbreviation first
    let expanded = expand_abbreviation(abbreviation)?;
    let mut results = Vec::with_capacity(selections.len());
    for &sel in selections {
        if let Some(close_idx) = expanded.rfind("</") {
            let mut result = String::with_capacity(expanded.len() + sel.len());
            result.push_str(&expanded[..close_idx]);
            result.push_str(sel);
            result.push_str(&expanded[close_idx..]);
            results.push(result);
        } else {
            return None;
        }
    }
    Some(results)
}

/// Returns `true` if `input` looks like a CSS abbreviation (a known CSS
/// property prefix optionally followed by a numeric value, or a keyword-only
/// abbreviation like `bgc`).
pub fn is_css_abbreviation(input: &str) -> bool {
    let input = input.trim();
    if input.is_empty() {
        return false;
    }
    // Keyword-only CSS abbreviations
    if matches!(input, "bgc" | "ff" | "fs" | "fw") {
        return true;
    }
    let prefixes = ["m", "p", "w", "h", "t", "b", "l", "r"];
    for prefix in prefixes {
        if let Some(rest) = input.strip_prefix(prefix) {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '-') {
                return true;
            }
        }
    }
    false
}

/// Extracts the Emmet abbreviation fragment immediately before `cursor_col`
/// in `line`. Returns `None` if no valid abbreviation characters precede the
/// cursor or if `cursor_col` is out of range.
pub fn extract_abbreviation_from_line(line: &str, cursor_col: usize) -> Option<&str> {
    if cursor_col == 0 || cursor_col > line.len() {
        return None;
    }
    let before = &line[..cursor_col];
    let start = before
        .rfind(|c: char| !c.is_alphanumeric() && !".#>{}+*".contains(c))
        .map(|i| i + 1)
        .unwrap_or(0);
    let abbr = &before[start..];
    if abbr.is_empty() {
        return None;
    }
    Some(abbr)
}

/// Counts the number of HTML element tags (`<tagname…>`) in `expanded`,
/// ignoring closing tags.
pub fn count_elements_in_expansion(expanded: &str) -> usize {
    let mut count = 0usize;
    let mut chars = expanded.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            if chars.peek() != Some(&'/') {
                count += 1;
            }
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Sibling combinator expansion
// ---------------------------------------------------------------------------

/// Expand an abbreviation containing sibling combinators (`+`).
///
/// For example, `"h1+p+footer"` expands to `<h1></h1>\n<p></p>\n<footer></footer>`.
pub fn expand_sibling_abbreviation(input: &str) -> Option<String> {
    if input.is_empty() || !input.contains('+') {
        return expand_abbreviation(input);
    }
    let parts: Vec<&str> = input.split('+').collect();
    let mut result = Vec::new();
    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            return None;
        }
        result.push(expand_abbreviation(trimmed)?);
    }
    Some(result.join("\n"))
}

/// Expand an abbreviation containing grouping with parentheses.
///
/// Groups are treated as sub-expressions: `"div>(h1+p)"` expands the group
/// inside the parent element.
pub fn expand_grouped_abbreviation(input: &str) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    // Simple case: no grouping
    if !input.contains('(') {
        return expand_sibling_abbreviation(input);
    }
    let open = input.find('(')?;
    let close = input.rfind(')')?;
    if close <= open {
        return None;
    }
    let parent_part = &input[..open];
    let group_part = &input[open + 1..close];

    let parent_abbr = parent_part.trim_end_matches('>');
    if parent_abbr.is_empty() {
        return expand_sibling_abbreviation(group_part);
    }

    let parent_expanded = expand_abbreviation(parent_abbr)?;
    let group_expanded = expand_sibling_abbreviation(group_part)?;

    // Insert group inside parent tag
    let close_tag = format!("</{parent_abbr}>");
    if let Some(idx) = parent_expanded.find(&close_tag) {
        let indented = group_expanded
            .lines()
            .map(|l| format!("  {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut result = parent_expanded[..idx].to_string();
        result.push('\n');
        result.push_str(&indented);
        result.push('\n');
        result.push_str(&parent_expanded[idx..]);
        Some(result)
    } else {
        Some(format!("{parent_expanded}\n{group_expanded}"))
    }
}

// ---------------------------------------------------------------------------
// Abbreviation validation
// ---------------------------------------------------------------------------

/// Errors returned by abbreviation validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbbreviationError {
    /// The abbreviation is empty.
    Empty,
    /// The abbreviation contains invalid characters.
    InvalidChar(char),
    /// Parentheses are unbalanced.
    UnbalancedParens,
    /// The abbreviation exceeds the maximum allowed length.
    TooLong { max: usize, actual: usize },
}

impl fmt::Display for AbbreviationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AbbreviationError::Empty => write!(f, "abbreviation is empty"),
            AbbreviationError::InvalidChar(c) => write!(f, "invalid character: '{c}'"),
            AbbreviationError::UnbalancedParens => write!(f, "unbalanced parentheses"),
            AbbreviationError::TooLong { max, actual } => {
                write!(f, "abbreviation length {actual} exceeds maximum {max}")
            }
        }
    }
}

impl std::error::Error for AbbreviationError {}

/// Validate an Emmet abbreviation for well-formedness.
///
/// Checks that the abbreviation is non-empty, contains only valid characters,
/// has balanced parentheses, and does not exceed `max_len`.
pub fn validate_abbreviation(input: &str, max_len: usize) -> Result<(), AbbreviationError> {
    if input.is_empty() {
        return Err(AbbreviationError::Empty);
    }
    if input.len() > max_len {
        return Err(AbbreviationError::TooLong {
            max: max_len,
            actual: input.len(),
        });
    }
    let allowed = |c: char| {
        c.is_alphanumeric()
            || matches!(c, '>' | '+' | '^' | '*' | '(' | ')' | '#' | '.' | '[' | ']' | '{' | '}' | '-' | '_' | ':' | '$' | '@' | '!' | '=' | '"' | '\'' | ' ')
    };
    let mut depth: i32 = 0;
    for c in input.chars() {
        if !allowed(c) {
            return Err(AbbreviationError::InvalidChar(c));
        }
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth < 0 {
                return Err(AbbreviationError::UnbalancedParens);
            }
        }
    }
    if depth != 0 {
        return Err(AbbreviationError::UnbalancedParens);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Expansion statistics (aggregate)
// ---------------------------------------------------------------------------

/// Accumulated statistics across multiple Emmet expansions.
#[derive(Debug, Clone)]
pub struct ExpansionSummary {
    pub total_expansions: u64,
    pub total_failures: u64,
    pub unique_tags: Vec<String>,
}

impl ExpansionSummary {
    /// Create a new empty summary.
    pub fn new() -> Self {
        Self {
            total_expansions: 0,
            total_failures: 0,
            unique_tags: Vec::new(),
        }
    }

    /// Record a successful expansion, tracking the tag name.
    pub fn record_expansion(&mut self, tag: &str) {
        self.total_expansions += 1;
        if !self.unique_tags.iter().any(|t| t == tag) {
            self.unique_tags.push(tag.to_string());
        }
    }

    /// Record a failed expansion attempt.
    pub fn record_failure(&mut self) {
        self.total_failures += 1;
    }

    /// Success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        let total = self.total_expansions + self.total_failures;
        if total == 0 {
            return 0.0;
        }
        self.total_expansions as f64 / total as f64
    }

    /// Number of distinct tags expanded.
    pub fn unique_tag_count(&self) -> usize {
        self.unique_tags.len()
    }
}

impl Default for ExpansionSummary {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExpansionSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} expansions, {} failures, {} unique tags",
            self.total_expansions,
            self.total_failures,
            self.unique_tags.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// Emmet snippet library – reusable named abbreviations
// ---------------------------------------------------------------------------

/// A named Emmet snippet that stores a frequently-used abbreviation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmmetSnippet {
    pub name: String,
    pub abbreviation: String,
    pub description: Option<String>,
    pub language: String,
}

impl EmmetSnippet {
    /// Create a new snippet.
    pub fn new(
        name: impl Into<String>,
        abbreviation: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            abbreviation: abbreviation.into(),
            description: None,
            language: language.into(),
        }
    }

    /// Set description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Expand the snippet's abbreviation.
    pub fn expand(&self) -> Option<String> {
        expand_abbreviation(&self.abbreviation)
    }

    /// Returns true if this snippet is for the given language.
    pub fn matches_language(&self, lang: &str) -> bool {
        self.language.eq_ignore_ascii_case(lang)
    }
}

impl fmt::Display for EmmetSnippet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}) → {}", self.name, self.language, self.abbreviation)
    }
}

/// A library of reusable Emmet snippets with search and filtering.
#[derive(Debug, Clone, Default)]
pub struct EmmetSnippetLibrary {
    snippets: Vec<EmmetSnippet>,
}

impl EmmetSnippetLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a snippet. Returns false if a snippet with the same name already exists.
    pub fn add(&mut self, snippet: EmmetSnippet) -> bool {
        if self.snippets.iter().any(|s| s.name == snippet.name) {
            return false;
        }
        self.snippets.push(snippet);
        true
    }

    /// Remove a snippet by name.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.snippets.len();
        self.snippets.retain(|s| s.name != name);
        self.snippets.len() < before
    }

    /// Look up a snippet by name.
    pub fn get(&self, name: &str) -> Option<&EmmetSnippet> {
        self.snippets.iter().find(|s| s.name == name)
    }

    /// Return all snippets for a given language.
    pub fn for_language(&self, lang: &str) -> Vec<&EmmetSnippet> {
        self.snippets.iter().filter(|s| s.matches_language(lang)).collect()
    }

    /// Search snippets by name substring (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&EmmetSnippet> {
        let q = query.to_lowercase();
        self.snippets
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&q))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.snippets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snippets.is_empty()
    }

    /// Return all unique languages in the library.
    pub fn languages(&self) -> Vec<&str> {
        let mut langs: Vec<&str> = self.snippets.iter().map(|s| s.language.as_str()).collect();
        langs.sort_unstable();
        langs.dedup();
        langs
    }
}

impl fmt::Display for EmmetSnippetLibrary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EmmetSnippetLibrary({} snippets)", self.snippets.len())
    }
}

// ---------------------------------------------------------------------------
// EmmetBalanceTag – find matching open/close tag pairs in HTML
// ---------------------------------------------------------------------------

/// Utilities for finding matching HTML tag pairs (balance inward/outward).
pub struct EmmetBalanceTag;

impl EmmetBalanceTag {
    /// Extracts the tag name starting at `pos` in `html`.
    ///
    /// `pos` must point to the `<` character of an opening or closing tag.
    /// Returns `None` if the position is out of range or not a valid tag start.
    pub fn extract_tag_name(html: &str, pos: usize) -> Option<String> {
        let bytes = html.as_bytes();
        if pos >= bytes.len() || bytes[pos] != b'<' {
            return None;
        }
        let mut start = pos + 1;
        // Skip `/` for closing tags
        if start < bytes.len() && bytes[start] == b'/' {
            start += 1;
        }
        let mut end = start;
        while end < bytes.len() {
            let ch = bytes[end];
            if ch == b' ' || ch == b'>' || ch == b'/' || ch == b'\n' || ch == b'\r' {
                break;
            }
            end += 1;
        }
        if end == start {
            return None;
        }
        let name = &html[start..end];
        if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            Some(name.to_ascii_lowercase())
        } else {
            None
        }
    }

    /// Finds the byte position of the matching closing tag for an opening tag
    /// at `open_pos`.
    ///
    /// The search handles nested tags of the same name and returns the position
    /// of the `<` in the closing tag, or `None` if no match is found.
    pub fn find_matching_close(html: &str, open_pos: usize) -> Option<usize> {
        let tag_name = Self::extract_tag_name(html, open_pos)?;

        // Check if this is a self-closing tag
        if self_closing_tags().contains(&tag_name.as_str()) {
            return None;
        }

        let search_start = open_pos + 1;
        let mut depth: usize = 1;
        let mut i = search_start;
        let bytes = html.as_bytes();

        while i < bytes.len() {
            if bytes[i] == b'<' {
                if let Some(name) = Self::extract_tag_name(html, i) {
                    if name == tag_name {
                        if i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                            depth -= 1;
                            if depth == 0 {
                                return Some(i);
                            }
                        } else {
                            depth += 1;
                        }
                    }
                }
            }
            i += 1;
        }
        None
    }

    /// Finds the byte position of the matching opening tag for a closing tag
    /// at `close_pos`.
    ///
    /// `close_pos` must point to the `<` of a `</tag>` sequence. Returns the
    /// position of the `<` in the corresponding opening tag, or `None`.
    pub fn find_matching_open(html: &str, close_pos: usize) -> Option<usize> {
        let bytes = html.as_bytes();
        if close_pos >= bytes.len() || bytes[close_pos] != b'<' {
            return None;
        }
        if close_pos + 1 >= bytes.len() || bytes[close_pos + 1] != b'/' {
            return None;
        }
        let tag_name = Self::extract_tag_name(html, close_pos)?;

        let mut depth: usize = 1;
        let mut i = close_pos;

        while i > 0 {
            i -= 1;
            if bytes[i] == b'<' {
                if let Some(name) = Self::extract_tag_name(html, i) {
                    if name == tag_name {
                        if i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                            depth += 1;
                        } else {
                            depth -= 1;
                            if depth == 0 {
                                return Some(i);
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl fmt::Display for EmmetBalanceTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EmmetBalanceTag")
    }
}

// ---------------------------------------------------------------------------
// EmmetMergeLines – merge multiple lines into a single line
// ---------------------------------------------------------------------------

/// Merges multiple text lines into a single string, trimming surrounding
/// whitespace from each line.
pub struct EmmetMergeLines;

impl EmmetMergeLines {
    /// Merges `lines` into one string separated by a single space, trimming
    /// leading/trailing whitespace from each line and skipping empty entries.
    pub fn merge(lines: &[&str]) -> String {
        Self::merge_with_separator(lines, " ")
    }

    /// Merges `lines` into one string using the given `sep`arator, trimming
    /// leading/trailing whitespace from each line and skipping empty entries.
    pub fn merge_with_separator(lines: &[&str], sep: &str) -> String {
        lines
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<&str>>()
            .join(sep)
    }
}

impl fmt::Display for EmmetMergeLines {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EmmetMergeLines")
    }
}

// ---------------------------------------------------------------------------
// EmmetUpdateImageSize – update width/height on <img> tags
// ---------------------------------------------------------------------------

/// Parses `<img>` tags and updates or extracts `width` / `height` attributes.
pub struct EmmetUpdateImageSize;

impl EmmetUpdateImageSize {
    /// Returns the `<img>` tag with `width` and `height` attributes set to the
    /// given values. Existing `width`/`height` attributes are replaced;
    /// missing ones are inserted before the closing `>` or `/>`.
    pub fn update_dimensions(img_tag: &str, width: u32, height: u32) -> String {
        let mut result = img_tag.to_string();
        result = Self::set_attr(&result, "width", &width.to_string());
        result = Self::set_attr(&result, "height", &height.to_string());
        result
    }

    /// Extracts the current `width` and `height` attribute values from an
    /// `<img>` tag, returning `None` if either attribute is missing or
    /// non-numeric.
    pub fn extract_dimensions(img_tag: &str) -> Option<(u32, u32)> {
        let w = Self::get_attr(img_tag, "width")?;
        let h = Self::get_attr(img_tag, "height")?;
        let w: u32 = w.parse().ok()?;
        let h: u32 = h.parse().ok()?;
        Some((w, h))
    }

    fn get_attr(tag: &str, attr_name: &str) -> Option<String> {
        let search = format!("{}=\"", attr_name);
        let start = tag.find(&search)?;
        let val_start = start + search.len();
        let val_end = tag[val_start..].find('"')? + val_start;
        Some(tag[val_start..val_end].to_string())
    }

    fn set_attr(tag: &str, attr_name: &str, value: &str) -> String {
        let search = format!("{}=\"", attr_name);
        if let Some(start) = tag.find(&search) {
            let val_start = start + search.len();
            if let Some(rel_end) = tag[val_start..].find('"') {
                let val_end = val_start + rel_end;
                let mut result = String::with_capacity(tag.len());
                result.push_str(&tag[..val_start]);
                result.push_str(value);
                result.push_str(&tag[val_end..]);
                return result;
            }
        }
        // Attribute not present – insert before closing > or />
        let new_attr = format!(" {}=\"{}\"", attr_name, value);
        if let Some(pos) = tag.rfind("/>") {
            let mut result = String::with_capacity(tag.len() + new_attr.len());
            result.push_str(&tag[..pos]);
            result.push_str(&new_attr);
            result.push_str(&tag[pos..]);
            result
        } else if let Some(pos) = tag.rfind('>') {
            let mut result = String::with_capacity(tag.len() + new_attr.len());
            result.push_str(&tag[..pos]);
            result.push_str(&new_attr);
            result.push_str(&tag[pos..]);
            result
        } else {
            format!("{}{}", tag, new_attr)
        }
    }
}

impl fmt::Display for EmmetUpdateImageSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EmmetUpdateImageSize")
    }
}

// ---------------------------------------------------------------------------
// EmmetMathExpression – multiplier expressions (tag*N) and numbering
// ---------------------------------------------------------------------------

/// Evaluates simple multiplier expressions found in Emmet abbreviations
/// (e.g. `li*5`) and expands tags with sequential numbering.
pub struct EmmetMathExpression;

impl EmmetMathExpression {
    /// Parses an input of the form `"tag*N"` and returns the tag name together
    /// with the repetition count. Returns `None` when the input does not
    /// contain a valid multiplier expression.
    pub fn evaluate_multiplier(input: &str) -> Option<(String, usize)> {
        let parts: Vec<&str> = input.splitn(2, '*').collect();
        if parts.len() != 2 {
            return None;
        }
        let tag = parts[0].trim();
        let count_str = parts[1].trim();
        if tag.is_empty() || count_str.is_empty() {
            return None;
        }
        let count: usize = count_str.parse().ok()?;
        if count == 0 {
            return None;
        }
        Some((tag.to_string(), count))
    }

    /// Expands a tag abbreviation into `count` numbered HTML elements.
    ///
    /// If the tag abbreviation contains `$`, each `$` is replaced with the
    /// 1-based item number. Otherwise the number is appended to the tag name
    /// as a class.
    pub fn expand_with_numbering(tag: &str, count: usize) -> Vec<String> {
        let mut result = Vec::with_capacity(count);
        let has_placeholder = tag.contains('$');

        for i in 1..=count {
            if has_placeholder {
                let expanded = tag.replace('$', &i.to_string());
                if let Some(html) = expand_abbreviation(&expanded) {
                    result.push(html);
                } else {
                    result.push(format!("<{0}></{0}>", expanded));
                }
            } else {
                let class_tag = format!("{}.item{}", tag, i);
                if let Some(html) = expand_abbreviation(&class_tag) {
                    result.push(html);
                } else {
                    result.push(format!("<{0} class=\"item{1}\"></{0}>", tag, i));
                }
            }
        }
        result
    }
}

impl fmt::Display for EmmetMathExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EmmetMathExpression")
    }
}

// ---------------------------------------------------------------------------
// expand_lorem – generate lorem ipsum placeholder text
// ---------------------------------------------------------------------------

/// The canonical lorem ipsum word pool used for placeholder text generation.
const LOREM_WORDS: &[&str] = &[
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing",
    "elit", "sed", "do", "eiusmod", "tempor", "incididunt", "ut", "labore",
    "et", "dolore", "magna", "aliqua", "enim", "ad", "minim", "veniam",
    "quis", "nostrud", "exercitation", "ullamco", "laboris", "nisi",
    "aliquip", "ex", "ea", "commodo", "consequat", "duis", "aute", "irure",
    "in", "reprehenderit", "voluptate", "velit", "esse", "cillum",
    "fugiat", "nulla", "pariatur", "excepteur", "sint", "occaecat",
    "cupidatat", "non", "proident", "sunt", "culpa", "qui", "officia",
    "deserunt", "mollit", "anim", "id", "est", "laborum",
];

/// Generates lorem ipsum placeholder text containing exactly `word_count`
/// words, cycling through the canonical word pool as needed.
///
/// Returns an empty string when `word_count` is zero.
pub fn expand_lorem(word_count: usize) -> String {
    if word_count == 0 {
        return String::new();
    }
    let pool_len = LOREM_WORDS.len();
    let mut words: Vec<&str> = Vec::with_capacity(word_count);
    for i in 0..word_count {
        words.push(LOREM_WORDS[i % pool_len]);
    }
    // Capitalise the first word and append a period at the end.
    let mut text = words.join(" ");
    if let Some(first) = text.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    text.push('.');
    text
}

// ---------------------------------------------------------------------------
// EmmetAbbreviationValidator - emmet abbreviation validator
// ---------------------------------------------------------------------------

/// Severity level for emmet abbreviation validator issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EmmetAbbreviationValidatorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for EmmetAbbreviationValidatorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [EmmetAbbreviationValidator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmmetAbbreviationValidatorEntry {
    pub id: String,
    pub label: String,
    pub severity: EmmetAbbreviationValidatorSeverity,
    pub detail: Option<String>,
    pub tag_count: usize,
    enabled: bool,
}

impl EmmetAbbreviationValidatorEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: EmmetAbbreviationValidatorSeverity::Low,
            detail: None,
            tag_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: EmmetAbbreviationValidatorSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_tag_count(mut self, val: usize) -> Self {
        self.tag_count = val;
        self
    }

    pub fn is_valid_abbrev(&self) -> bool {
        self.enabled && self.severity >= EmmetAbbreviationValidatorSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.tag_count, det)
    }
}

impl fmt::Display for EmmetAbbreviationValidatorEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [EmmetAbbreviationValidatorEntry] items.
#[derive(Debug, Clone)]
pub struct EmmetAbbreviationValidator {
    entries: Vec<EmmetAbbreviationValidatorEntry>,
    name: String,
    capacity: usize,
}

impl EmmetAbbreviationValidator {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: EmmetAbbreviationValidatorEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<EmmetAbbreviationValidatorEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&EmmetAbbreviationValidatorEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn tag_count(&self) -> usize { self.entries.len() }

    pub fn is_valid_abbrev(&self) -> bool {
        self.entries.iter().any(|e| e.is_valid_abbrev())
    }

    pub fn entries_by_severity(&self, severity: EmmetAbbreviationValidatorSeverity) -> Vec<&EmmetAbbreviationValidatorEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= EmmetAbbreviationValidatorSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&EmmetAbbreviationValidatorEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&EmmetAbbreviationValidatorEntry> {
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
// EmmetOutputFormatter - emmet output formatter
// ---------------------------------------------------------------------------

/// Configuration for [EmmetOutputFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmmetOutputFormatterConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub nesting_depth: usize,
}

impl EmmetOutputFormatterConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, nesting_depth: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_nesting_depth(mut self, val: usize) -> Self { self.nesting_depth = val; self }
}

impl Default for EmmetOutputFormatterConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [EmmetOutputFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmmetOutputFormatterItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl EmmetOutputFormatterItem {
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

    pub fn needs_formatting(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for EmmetOutputFormatterItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [EmmetOutputFormatterItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct EmmetOutputFormatter {
    config: EmmetOutputFormatterConfig,
    items: Vec<EmmetOutputFormatterItem>,
}

impl EmmetOutputFormatter {
    pub fn new(config: EmmetOutputFormatterConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: EmmetOutputFormatterItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<EmmetOutputFormatterItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&EmmetOutputFormatterItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn nesting_depth(&self) -> usize { self.items.len() }

    pub fn needs_formatting(&self) -> bool {
        self.items.iter().any(|i| i.needs_formatting())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&EmmetOutputFormatterItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&EmmetOutputFormatterItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &EmmetOutputFormatterConfig {
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
// vsedit-emmet: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmmetXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl EmmetXConfig {
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

impl std::fmt::Display for EmmetXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct EmmetXRegistry {
    entries: Vec<EmmetXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl EmmetXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: EmmetXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&EmmetXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut EmmetXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<EmmetXConfig> {
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

    pub fn active_entries(&self) -> Vec<&EmmetXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&EmmetXConfig> {
        let mut sorted: Vec<&EmmetXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&EmmetXConfig> {
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

    pub fn iter(&self) -> EmmetXIterator<'_> {
        EmmetXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct EmmetXIterator<'a> {
    inner: std::slice::Iter<'a, EmmetXConfig>,
}

impl<'a> Iterator for EmmetXIterator<'a> {
    type Item = &'a EmmetXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct EmmetXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl EmmetXCache {
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
pub struct EmmetXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl EmmetXFormatter {
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

    pub fn format_entry(&self, entry: &EmmetXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &EmmetXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &EmmetXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for EmmetXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct EmmetXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl EmmetXValidator {
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

    pub fn validate(&self, entry: &EmmetXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &EmmetXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for EmmetXValidator {
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
// xc_ pool and scheduler – generated block 42
// ---------------------------------------------------------------------------

/// Generic object pool `Xc42Pool<T>`.
pub struct Xc42Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc42Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc42PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc42Pool<T> {
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
    pub fn stats(&self) -> Xc42PoolStats {
        Xc42PoolStats {
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

impl<T> Default for Xc42Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc42Scheduler`.
pub struct Xc42Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc42Scheduler {
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

impl Default for Xc42Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_42 hash for the given byte slice.
pub fn xc_42_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_42 convention.
pub fn xc_42_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_11 deepening: state machine + event bus ---

/// States for the Xd11 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd11State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd11State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd11Transition {
    pub from: Xd11State,
    pub to: Xd11State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd11StateMachine {
    current: Xd11State,
    history: Vec<Xd11Transition>,
    step_counter: usize,
}

impl Xd11StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd11State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd11State {
        self.current
    }

    pub fn history(&self) -> &[Xd11Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd11State) -> Result<Xd11State, String> {
        let allowed = match (self.current, target) {
            (Xd11State::Idle, Xd11State::Running) => true,
            (Xd11State::Running, Xd11State::Paused) => true,
            (Xd11State::Running, Xd11State::Done) => true,
            (Xd11State::Paused, Xd11State::Running) => true,
            (Xd11State::Paused, Xd11State::Done) => true,
            (Xd11State::Done, Xd11State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_11: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd11Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd11SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd11State> {
        let prefix = "Xd11SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd11State::Idle),
            "Running" => Some(Xd11State::Running),
            "Paused" => Some(Xd11State::Paused),
            "Done" => Some(Xd11State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd11State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd11 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd11Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd11Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd11HandlerFn = Box<dyn Fn(&Xd11Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd11EventBus {
    handlers: Vec<(usize, Option<String>, Xd11HandlerFn)>,
    next_id: usize,
    published: Vec<Xd11Event>,
}

impl Xd11EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd11Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd11Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd11Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd11Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
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

    #[test]
    fn expand_multiplication() {
        assert_eq!(
            expand_abbreviation("li*3"),
            Some("<li></li>\n<li></li>\n<li></li>".to_string()),
        );
    }

    #[test]
    fn expand_multiplication_single() {
        assert_eq!(
            expand_abbreviation("p*1"),
            Some("<p></p>".to_string()),
        );
    }

    #[test]
    fn expand_multiplication_zero_returns_none() {
        assert_eq!(expand_abbreviation("p*0"), None);
    }

    #[test]
    fn expand_siblings() {
        assert_eq!(
            expand_abbreviation("header+main+footer"),
            Some("<header></header>\n<main></main>\n<footer></footer>".to_string()),
        );
    }

    #[test]
    fn expand_css_margin() {
        assert_eq!(
            expand_css_abbreviation("m10"),
            Some("margin: 10px;".to_string()),
        );
    }

    #[test]
    fn expand_css_padding() {
        assert_eq!(
            expand_css_abbreviation("p20"),
            Some("padding: 20px;".to_string()),
        );
    }

    #[test]
    fn expand_css_width() {
        assert_eq!(
            expand_css_abbreviation("w100"),
            Some("width: 100px;".to_string()),
        );
    }

    #[test]
    fn expand_css_bgc() {
        assert_eq!(
            expand_css_abbreviation("bgc"),
            Some("background-color: ;".to_string()),
        );
    }

    #[test]
    fn expand_css_invalid() {
        assert_eq!(expand_css_abbreviation(""), None);
        assert_eq!(expand_css_abbreviation("zzz"), None);
    }

    #[test]
    fn wrap_abbreviation_basic() {
        assert_eq!(
            get_wrap_abbreviation("Hello", "div"),
            Some("<div>Hello</div>".to_string()),
        );
    }

    #[test]
    fn wrap_abbreviation_with_class() {
        assert_eq!(
            get_wrap_abbreviation("text", "p.note"),
            Some("<p class=\"note\">text</p>".to_string()),
        );
    }

    #[test]
    fn self_closing_tags_list() {
        let tags = self_closing_tags();
        assert!(tags.contains(&"img"));
        assert!(tags.contains(&"br"));
        assert!(tags.contains(&"hr"));
        assert!(tags.contains(&"input"));
        assert!(tags.contains(&"meta"));
        assert!(tags.contains(&"link"));
        assert_eq!(tags.len(), 6);
    }

    #[test]
    fn config_syntax_management() {
        let mut cfg = EmmetConfig::default();
        assert!(cfg.is_syntax_supported("html"));
        assert!(!cfg.is_syntax_supported("jsx"));

        cfg.add_syntax("jsx");
        assert!(cfg.is_syntax_supported("jsx"));

        cfg.remove_syntax("jsx");
        assert!(!cfg.is_syntax_supported("jsx"));

        let before = cfg.syntaxes.len();
        cfg.add_syntax("html");
        assert_eq!(cfg.syntaxes.len(), before);
    }

    #[test]
    fn expand_with_config_disabled() {
        let mut cfg = EmmetConfig::default();
        cfg.enabled = false;
        assert_eq!(expand_abbreviation_with_config("div", &cfg), None);
    }

    #[test]
    fn expand_with_config_unsupported_syntax() {
        let mut cfg = EmmetConfig::default();
        cfg.syntaxes = vec!["css".to_string()];
        assert_eq!(expand_abbreviation_with_config("div", &cfg), None);
    }

    #[test]
    fn expand_with_config_ok() {
        let cfg = EmmetConfig::default();
        assert_eq!(
            expand_abbreviation_with_config("div", &cfg),
            Some("<div></div>".to_string()),
        );
    }

    #[test]
    fn emmet_action_variants() {
        let actions = [
            EmmetAction::Expand,
            EmmetAction::Wrap,
            EmmetAction::Balance,
            EmmetAction::GoToMatching,
        ];
        for a in &actions {
            let _ = format!("{a:?}");
            let _ = a.clone();
        }
    }

    #[test]
    fn eq_showexpanded_same() {
        assert_eq!(ShowExpanded::Always, ShowExpanded::Always);
    }

    #[test]
    fn ne_showexpanded_diff() {
        assert_ne!(ShowExpanded::Always, ShowExpanded::Never);
    }

    #[test]
    fn eq_emmetaction_same() {
        assert_eq!(EmmetAction::Expand, EmmetAction::Expand);
    }

    #[test]
    fn ne_emmetaction_diff() {
        assert_ne!(EmmetAction::Expand, EmmetAction::Wrap);
    }

    #[test]
    fn initial_is_syntax_supported() {
        let svc = EmmetConfig::default();
        let _val = svc.is_syntax_supported("html");
    }

    #[test]
    fn parser_simple_tag() {
        let parser = EmmetAbbreviationParser::new("div");
        assert!(parser.is_valid());
        let node = parser.parse().unwrap();
        assert_eq!(
            node,
            AbbreviationNode::Tag {
                name: "div".to_string(),
                children: Vec::new(),
            }
        );
        assert_eq!(node.render(), "<div></div>");
    }

    #[test]
    fn parser_parent_child() {
        let parser = EmmetAbbreviationParser::new("ul>li");
        let node = parser.parse().unwrap();
        assert_eq!(node.render(), "<ul>\n  <li></li>\n</ul>");
    }

    #[test]
    fn parser_sibling() {
        let parser = EmmetAbbreviationParser::new("h1+p+footer");
        let node = parser.parse().unwrap();
        assert_eq!(node.render(), "<h1></h1>\n<p></p>\n<footer></footer>");
    }

    #[test]
    fn parser_repeat() {
        let parser = EmmetAbbreviationParser::new("li*3");
        let node = parser.parse().unwrap();
        assert_eq!(node.render(), "<li></li>\n<li></li>\n<li></li>");
    }

    #[test]
    fn parser_invalid_input() {
        let parser = EmmetAbbreviationParser::new("");
        assert!(!parser.is_valid());
        assert!(parser.parse().is_none());

        let parser2 = EmmetAbbreviationParser::new("hello world");
        assert!(!parser2.is_valid());
        assert!(parser2.parse().is_none());
    }

    #[test]
    fn tag_completion_basic() {
        assert_eq!(tag_completion("<div"), Some("</div>".to_string()));
        assert_eq!(tag_completion("<span"), Some("</span>".to_string()));
        assert_eq!(tag_completion("<br"), None); // self-closing
        assert_eq!(tag_completion("<img"), None); // self-closing
        assert_eq!(tag_completion("div"), None); // no '<'
        assert_eq!(tag_completion("<"), None); // empty tag
        assert_eq!(tag_completion("</div"), None); // closing tag
    }

    #[test]
    fn needs_closing_tag_basic() {
        assert!(needs_closing_tag("div"));
        assert!(needs_closing_tag("span"));
        assert!(needs_closing_tag("p"));
        assert!(!needs_closing_tag("br"));
        assert!(!needs_closing_tag("img"));
        assert!(!needs_closing_tag("hr"));
        assert!(!needs_closing_tag("input"));
        assert!(!needs_closing_tag("meta"));
        assert!(!needs_closing_tag("link"));
    }

    #[test]
    fn close_tag_basic() {
        assert_eq!(close_tag("div"), "</div>");
        assert_eq!(close_tag("span"), "</span>");
    }

    #[test]
    fn wrap_with_abbreviation_basic() {
        let result = emmet_wrap_with_abbreviation(&["Hello", "World"], "div").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "<div>Hello</div>");
        assert_eq!(result[1], "<div>World</div>");
    }

    #[test]
    fn wrap_with_abbreviation_invalid() {
        assert!(emmet_wrap_with_abbreviation(&["text"], "").is_none());
    }

    #[test]
    fn wrap_with_abbreviation_nested() {
        let result = emmet_wrap_with_abbreviation(&["content"], "ul>li").unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("content"));
        assert!(result[0].contains("<ul>"));
        assert!(result[0].contains("</ul>"));
    }

    #[test]
    fn emmet_stats_new_defaults() {
        let stats = EmmetStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn emmet_stats_record_success() {
        let mut stats = EmmetStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn emmet_stats_record_failure() {
        let mut stats = EmmetStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn emmet_stats_reset() {
        let mut stats = EmmetStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn emmet_stats_merge() {
        let mut a = EmmetStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = EmmetStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn emmet_stats_display() {
        let mut stats = EmmetStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn emmet_stats_default() {
        let stats = EmmetStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn emmet_validator_accepts_valid_name() {
        let v = EmmetValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn emmet_validator_rejects_empty() {
        let v = EmmetValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn emmet_validator_rejects_too_long() {
        let v = EmmetValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn emmet_validator_forbidden_prefix() {
        let v = EmmetValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn emmet_validator_allowed_chars() {
        let v = EmmetValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn emmet_validator_range() {
        let v = EmmetValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn emmet_sanitize_removes_control() {
        let result = EmmetValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn emmet_truncate_short_string() {
        assert_eq!(EmmetValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn emmet_truncate_long_string() {
        let result = EmmetValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn emmet_is_ascii_printable() {
        assert!(EmmetValidator::is_ascii_printable("Hello World 123"));
        assert!(!EmmetValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn config_syntax_count() {
        let cfg = EmmetConfig::default();
        assert_eq!(cfg.syntax_count(), 3);
        let mut empty_cfg = cfg.clone();
        empty_cfg.syntaxes.clear();
        assert_eq!(empty_cfg.syntax_count(), 0);
    }

    #[test]
    fn config_has_suggestions_enabled() {
        let mut cfg = EmmetConfig::default();
        assert!(cfg.has_suggestions_enabled());
        cfg.toggle_show_abbreviation_suggestions();
        assert!(!cfg.has_suggestions_enabled());
    }

    #[test]
    fn action_label() {
        assert_eq!(EmmetAction::Expand.label(), "Expand Abbreviation");
        assert_eq!(EmmetAction::Wrap.label(), "Wrap with Abbreviation");
        assert_eq!(EmmetAction::Balance.label(), "Balance (Select Matching)");
        assert_eq!(EmmetAction::GoToMatching.label(), "Go to Matching Pair");
    }

    #[test]
    fn action_is_expand() {
        assert!(EmmetAction::Expand.is_expand());
        assert!(!EmmetAction::Wrap.is_expand());
        assert!(!EmmetAction::Balance.is_expand());
        assert!(!EmmetAction::GoToMatching.is_expand());
    }

    #[test]
    fn is_css_abbreviation_valid() {
        assert!(is_css_abbreviation("m10"));
        assert!(is_css_abbreviation("p20"));
        assert!(is_css_abbreviation("w100"));
        assert!(is_css_abbreviation("bgc"));
        assert!(is_css_abbreviation("ff"));
        assert!(is_css_abbreviation("h-5"));
    }

    #[test]
    fn is_css_abbreviation_invalid() {
        assert!(!is_css_abbreviation(""));
        assert!(!is_css_abbreviation("div"));
        assert!(!is_css_abbreviation("m"));
        assert!(!is_css_abbreviation("hello"));
    }

    #[test]
    fn extract_abbreviation_basic() {
        assert_eq!(
            extract_abbreviation_from_line("  div.foo", 9),
            Some("div.foo"),
        );
        assert_eq!(
            extract_abbreviation_from_line("  ul>li", 7),
            Some("ul>li"),
        );
        assert_eq!(extract_abbreviation_from_line("  div", 0), None);
        assert_eq!(extract_abbreviation_from_line("text", 100), None);
    }

    #[test]
    fn count_elements_in_expansion_basic() {
        assert_eq!(count_elements_in_expansion("<div></div>"), 1);
        assert_eq!(
            count_elements_in_expansion("<ul>\n  <li></li>\n</ul>"),
            2,
        );
        assert_eq!(
            count_elements_in_expansion("<h1></h1>\n<p></p>\n<footer></footer>"),
            3,
        );
        assert_eq!(count_elements_in_expansion(""), 0);
    }

    #[test]
    fn show_expanded_is_always() {
        assert!(ShowExpanded::Always.is_always());
        assert!(!ShowExpanded::Never.is_always());
        assert!(!ShowExpanded::InMarkupAndStylesheetFilesOnly.is_always());
    }

    // -- new tests --

    #[test]
    fn expand_sibling_h1_plus_p() {
        let result = expand_sibling_abbreviation("h1+p").unwrap();
        assert!(result.contains("<h1></h1>"));
        assert!(result.contains("<p></p>"));
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn expand_sibling_three_tags() {
        let result = expand_sibling_abbreviation("h1+p+footer").unwrap();
        assert!(result.contains("<h1></h1>"));
        assert!(result.contains("<p></p>"));
        assert!(result.contains("<footer></footer>"));
    }

    #[test]
    fn expand_sibling_single_tag_falls_back() {
        let result = expand_sibling_abbreviation("div").unwrap();
        assert_eq!(result, "<div></div>");
    }

    #[test]
    fn expand_sibling_empty_part_returns_none() {
        assert!(expand_sibling_abbreviation("div+").is_none());
    }

    #[test]
    fn expand_grouped_abbreviation_basic() {
        let result = expand_grouped_abbreviation("div>(h1+p)").unwrap();
        assert!(result.contains("<div>"));
        assert!(result.contains("<h1></h1>"));
        assert!(result.contains("<p></p>"));
        assert!(result.contains("</div>"));
    }

    #[test]
    fn expand_grouped_no_parens_delegates() {
        let result = expand_grouped_abbreviation("span").unwrap();
        assert_eq!(result, "<span></span>");
    }

    #[test]
    fn validate_abbreviation_ok() {
        assert!(validate_abbreviation("div>p+span", 100).is_ok());
        assert!(validate_abbreviation("ul>li*3", 100).is_ok());
    }

    #[test]
    fn validate_abbreviation_empty() {
        assert_eq!(validate_abbreviation("", 100), Err(AbbreviationError::Empty));
    }

    #[test]
    fn validate_abbreviation_too_long() {
        let long = "a".repeat(101);
        match validate_abbreviation(&long, 100) {
            Err(AbbreviationError::TooLong { max: 100, actual: 101 }) => {}
            other => panic!("expected TooLong, got {:?}", other),
        }
    }

    #[test]
    fn validate_abbreviation_unbalanced_parens() {
        assert_eq!(
            validate_abbreviation("div>(p", 100),
            Err(AbbreviationError::UnbalancedParens),
        );
        assert_eq!(
            validate_abbreviation("div>p)", 100),
            Err(AbbreviationError::UnbalancedParens),
        );
    }

    #[test]
    fn expansion_summary_tracks_tags() {
        let mut s = ExpansionSummary::new();
        s.record_expansion("div");
        s.record_expansion("p");
        s.record_expansion("div");
        assert_eq!(s.total_expansions, 3);
        assert_eq!(s.unique_tag_count(), 2);
        assert!((s.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn expansion_summary_with_failures() {
        let mut s = ExpansionSummary::new();
        s.record_expansion("div");
        s.record_failure();
        assert_eq!(s.total_failures, 1);
        assert!((s.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn expansion_summary_display() {
        let s = ExpansionSummary::new();
        let text = format!("{s}");
        assert!(text.contains("0 expansions"));
    }

    #[test]
    fn abbreviation_error_display() {
        assert_eq!(
            AbbreviationError::Empty.to_string(),
            "abbreviation is empty",
        );
        assert!(AbbreviationError::InvalidChar('~').to_string().contains('~'));
    }

    // --- new tests ---

    #[test]
    fn snippet_creation_and_expansion() {
        let snip = EmmetSnippet::new("boilerplate", "html>head+body", "html")
            .with_description("HTML boilerplate");
        assert_eq!(snip.name, "boilerplate");
        assert!(snip.matches_language("HTML"));
        assert!(snip.description.is_some());
        let expanded = snip.expand();
        assert!(expanded.is_some());
        let html = expanded.unwrap();
        assert!(html.contains("<html>"));
        assert!(html.contains("<head>"));
    }

    #[test]
    fn snippet_library_add_and_search() {
        let mut lib = EmmetSnippetLibrary::new();
        let s1 = EmmetSnippet::new("nav-bar", "nav>ul>li*3", "html");
        let s2 = EmmetSnippet::new("css-reset", "m0", "css");
        assert!(lib.add(s1));
        assert!(lib.add(s2));
        assert_eq!(lib.len(), 2);
        // duplicate rejected
        assert!(!lib.add(EmmetSnippet::new("nav-bar", "div", "html")));
        // search
        let results = lib.search("nav");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "nav-bar");
    }

    #[test]
    fn snippet_library_filter_by_language() {
        let mut lib = EmmetSnippetLibrary::new();
        lib.add(EmmetSnippet::new("a", "div", "html"));
        lib.add(EmmetSnippet::new("b", "span", "html"));
        lib.add(EmmetSnippet::new("c", "p0", "css"));
        assert_eq!(lib.for_language("html").len(), 2);
        assert_eq!(lib.for_language("css").len(), 1);
        assert_eq!(lib.for_language("xml").len(), 0);
    }

    #[test]
    fn snippet_library_languages() {
        let mut lib = EmmetSnippetLibrary::new();
        lib.add(EmmetSnippet::new("a", "div", "html"));
        lib.add(EmmetSnippet::new("b", "p0", "css"));
        lib.add(EmmetSnippet::new("c", "span", "html"));
        let langs = lib.languages();
        assert_eq!(langs, vec!["css", "html"]);
    }

    #[test]
    fn snippet_library_remove() {
        let mut lib = EmmetSnippetLibrary::new();
        lib.add(EmmetSnippet::new("x", "div", "html"));
        assert!(lib.remove("x"));
        assert!(!lib.remove("x"));
        assert!(lib.is_empty());
    }

    #[test]
    fn snippet_display() {
        let s = EmmetSnippet::new("test", "div>p", "html");
        let text = format!("{}", s);
        assert!(text.contains("test"));
        assert!(text.contains("html"));
        assert!(text.contains("div>p"));
    }

    // -----------------------------------------------------------------------
    // EmmetBalanceTag tests
    // -----------------------------------------------------------------------

    #[test]
    fn balance_extract_tag_name_open() {
        let html = "<div class=\"x\">";
        assert_eq!(
            EmmetBalanceTag::extract_tag_name(html, 0),
            Some("div".to_string()),
        );
    }

    #[test]
    fn balance_extract_tag_name_close() {
        let html = "</span>";
        assert_eq!(
            EmmetBalanceTag::extract_tag_name(html, 0),
            Some("span".to_string()),
        );
    }

    #[test]
    fn balance_find_matching_close_simple() {
        let html = "<div><p>hello</p></div>";
        assert_eq!(EmmetBalanceTag::find_matching_close(html, 0), Some(17));
    }

    #[test]
    fn balance_find_matching_close_nested() {
        let html = "<div><div>inner</div></div>";
        // The outermost <div> at 0 should match the last </div>
        assert_eq!(EmmetBalanceTag::find_matching_close(html, 0), Some(21));
    }

    #[test]
    fn balance_find_matching_open_simple() {
        let html = "<div><p>hello</p></div>";
        // </div> starts at position 17
        assert_eq!(EmmetBalanceTag::find_matching_open(html, 17), Some(0));
    }

    #[test]
    fn balance_self_closing_returns_none() {
        let html = "<img src=\"a.png\">";
        assert_eq!(EmmetBalanceTag::find_matching_close(html, 0), None);
    }

    // -----------------------------------------------------------------------
    // EmmetMergeLines tests
    // -----------------------------------------------------------------------

    #[test]
    fn merge_lines_basic() {
        let lines = vec!["  hello  ", "  world  "];
        assert_eq!(EmmetMergeLines::merge(&lines), "hello world");
    }

    #[test]
    fn merge_lines_with_separator() {
        let lines = vec!["a", " b ", "c"];
        assert_eq!(EmmetMergeLines::merge_with_separator(&lines, ", "), "a, b, c");
    }

    #[test]
    fn merge_lines_skips_empty() {
        let lines = vec!["a", "  ", "", "b"];
        assert_eq!(EmmetMergeLines::merge(&lines), "a b");
    }

    // -----------------------------------------------------------------------
    // EmmetUpdateImageSize tests
    // -----------------------------------------------------------------------

    #[test]
    fn update_image_dimensions_existing() {
        let tag = r#"<img src="a.png" width="10" height="20" />"#;
        let updated = EmmetUpdateImageSize::update_dimensions(tag, 100, 200);
        assert!(updated.contains("width=\"100\""));
        assert!(updated.contains("height=\"200\""));
    }

    #[test]
    fn update_image_dimensions_missing() {
        let tag = r#"<img src="a.png" />"#;
        let updated = EmmetUpdateImageSize::update_dimensions(tag, 50, 75);
        assert!(updated.contains("width=\"50\""));
        assert!(updated.contains("height=\"75\""));
    }

    #[test]
    fn extract_image_dimensions() {
        let tag = r#"<img src="a.png" width="320" height="240" />"#;
        assert_eq!(
            EmmetUpdateImageSize::extract_dimensions(tag),
            Some((320, 240)),
        );
    }

    #[test]
    fn extract_image_dimensions_missing() {
        let tag = r#"<img src="a.png" />"#;
        assert_eq!(EmmetUpdateImageSize::extract_dimensions(tag), None);
    }

    // -----------------------------------------------------------------------
    // EmmetMathExpression tests
    // -----------------------------------------------------------------------

    #[test]
    fn math_evaluate_multiplier() {
        assert_eq!(
            EmmetMathExpression::evaluate_multiplier("li*5"),
            Some(("li".to_string(), 5)),
        );
    }

    #[test]
    fn math_evaluate_multiplier_invalid() {
        assert_eq!(EmmetMathExpression::evaluate_multiplier("li"), None);
        assert_eq!(EmmetMathExpression::evaluate_multiplier("*5"), None);
        assert_eq!(EmmetMathExpression::evaluate_multiplier("li*0"), None);
    }

    #[test]
    fn math_expand_with_numbering() {
        let items = EmmetMathExpression::expand_with_numbering("li", 3);
        assert_eq!(items.len(), 3);
        assert!(items[0].contains("item1"));
        assert!(items[2].contains("item3"));
    }

    // -----------------------------------------------------------------------
    // expand_lorem tests
    // -----------------------------------------------------------------------

    #[test]
    fn lorem_word_count() {
        let text = expand_lorem(10);
        // The output ends with a period; split on whitespace to count words
        let count = text.trim_end_matches('.').split_whitespace().count();
        assert_eq!(count, 10);
    }

    #[test]
    fn lorem_zero_words() {
        assert_eq!(expand_lorem(0), "");
    }

    #[test]
    fn lorem_starts_capitalised() {
        let text = expand_lorem(5);
        assert!(text.starts_with('L'));
        assert!(text.ends_with('.'));
    }

#[test]
    fn emmetabbreviationvalidator_severity_ordering() {
        assert!(EmmetAbbreviationValidatorSeverity::Critical > EmmetAbbreviationValidatorSeverity::High);
        assert!(EmmetAbbreviationValidatorSeverity::High > EmmetAbbreviationValidatorSeverity::Medium);
        assert!(EmmetAbbreviationValidatorSeverity::Medium > EmmetAbbreviationValidatorSeverity::Low);
    }

    #[test]
    fn emmetabbreviationvalidator_severity_display() {
        assert_eq!(EmmetAbbreviationValidatorSeverity::Low.to_string(), "low");
        assert_eq!(EmmetAbbreviationValidatorSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn emmetabbreviationvalidator_entry_creation() {
        let e = EmmetAbbreviationValidatorEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, EmmetAbbreviationValidatorSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn emmetabbreviationvalidator_entry_builder() {
        let e = EmmetAbbreviationValidatorEntry::new("e2", "Entry 2")
            .with_severity(EmmetAbbreviationValidatorSeverity::High)
            .with_detail("some detail")
            .with_tag_count(42);
        assert_eq!(e.severity, EmmetAbbreviationValidatorSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.tag_count, 42);
    }

    #[test]
    fn emmetabbreviationvalidator_entry_enable_disable() {
        let mut e = EmmetAbbreviationValidatorEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn emmetabbreviationvalidator_add_and_count() {
        let mut mgr = EmmetAbbreviationValidator::new("test");
        mgr.add(EmmetAbbreviationValidatorEntry::new("a", "A"));
        mgr.add(EmmetAbbreviationValidatorEntry::new("b", "B").with_severity(EmmetAbbreviationValidatorSeverity::High));
        assert_eq!(mgr.tag_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn emmetabbreviationvalidator_remove() {
        let mut mgr = EmmetAbbreviationValidator::new("test");
        mgr.add(EmmetAbbreviationValidatorEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn emmetabbreviationvalidator_capacity() {
        let mut mgr = EmmetAbbreviationValidator::new("test").with_capacity(1);
        assert!(mgr.add(EmmetAbbreviationValidatorEntry::new("a", "A")));
        assert!(!mgr.add(EmmetAbbreviationValidatorEntry::new("b", "B")));
    }

    #[test]
    fn emmetabbreviationvalidator_sorted_by_severity() {
        let mut mgr = EmmetAbbreviationValidator::new("test");
        mgr.add(EmmetAbbreviationValidatorEntry::new("lo", "Low"));
        mgr.add(EmmetAbbreviationValidatorEntry::new("hi", "High").with_severity(EmmetAbbreviationValidatorSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, EmmetAbbreviationValidatorSeverity::Critical);
    }

    #[test]
    fn emmetabbreviationvalidator_summary() {
        let mgr = EmmetAbbreviationValidator::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn emmetoutputformatter_config_defaults() {
        let cfg = EmmetOutputFormatterConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn emmetoutputformatter_item_creation() {
        let item = EmmetOutputFormatterItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn emmetoutputformatter_add_and_get() {
        let mut mgr = EmmetOutputFormatter::new(EmmetOutputFormatterConfig::new("test"));
        mgr.add(EmmetOutputFormatterItem::new("k1", "v1"));
        assert_eq!(mgr.nesting_depth(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn emmetoutputformatter_remove_item() {
        let mut mgr = EmmetOutputFormatter::new(EmmetOutputFormatterConfig::new("test"));
        mgr.add(EmmetOutputFormatterItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn emmetoutputformatter_sorted_by_priority() {
        let mut mgr = EmmetOutputFormatter::new(EmmetOutputFormatterConfig::new("test"));
        mgr.add(EmmetOutputFormatterItem::new("lo", "low").with_priority(1));
        mgr.add(EmmetOutputFormatterItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn emmetoutputformatter_items_with_tag() {
        let mut mgr = EmmetOutputFormatter::new(EmmetOutputFormatterConfig::new("test"));
        mgr.add(EmmetOutputFormatterItem::new("a", "1").with_tag("x"));
        mgr.add(EmmetOutputFormatterItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn emmetoutputformatter_report() {
        let mgr = EmmetOutputFormatter::new(EmmetOutputFormatterConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn emmet_x_config_new() {
        let c = EmmetXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn emmet_x_config_builder() {
        let c = EmmetXConfig::new("k")
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
    fn emmet_x_config_display() {
        let c = EmmetXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn emmet_x_registry_insert_get() {
        let mut reg = EmmetXRegistry::new();
        reg.insert(EmmetXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn emmet_x_registry_duplicate() {
        let mut reg = EmmetXRegistry::new();
        reg.insert(EmmetXConfig::new("a")).unwrap();
        assert!(reg.insert(EmmetXConfig::new("a")).is_err());
    }

    #[test]
    fn emmet_x_registry_remove() {
        let mut reg = EmmetXRegistry::new();
        reg.insert(EmmetXConfig::new("a")).unwrap();
        reg.insert(EmmetXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn emmet_x_registry_active_entries() {
        let mut reg = EmmetXRegistry::new();
        reg.insert(EmmetXConfig::new("a")).unwrap();
        reg.insert(EmmetXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn emmet_x_registry_by_weight() {
        let mut reg = EmmetXRegistry::new();
        reg.insert(EmmetXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(EmmetXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn emmet_x_registry_tags() {
        let mut reg = EmmetXRegistry::new();
        reg.insert(EmmetXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(EmmetXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn emmet_x_registry_total_weight() {
        let mut reg = EmmetXRegistry::new();
        reg.insert(EmmetXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(EmmetXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn emmet_x_registry_iterator() {
        let mut reg = EmmetXRegistry::new();
        reg.insert(EmmetXConfig::new("a")).unwrap();
        reg.insert(EmmetXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn emmet_x_cache_put_get() {
        let mut cache = EmmetXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn emmet_x_cache_eviction() {
        let mut cache = EmmetXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn emmet_x_cache_lru_order() {
        let mut cache = EmmetXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn emmet_x_cache_most_least_recent() {
        let mut cache = EmmetXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn emmet_x_formatter_entry() {
        let e = EmmetXConfig::new("k").with_value("v");
        let fmt = EmmetXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn emmet_x_formatter_summary() {
        let mut reg = EmmetXRegistry::new();
        reg.insert(EmmetXConfig::new("a").with_weight(5)).unwrap();
        let fmt = EmmetXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn emmet_x_validator_valid() {
        let v = EmmetXValidator::new();
        let c = EmmetXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn emmet_x_validator_empty_key() {
        let v = EmmetXValidator::new();
        let c = EmmetXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn emmet_x_validator_require_value() {
        let v = EmmetXValidator::new().require_value(true);
        let c = EmmetXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn emmet_x_validator_allowed_tags() {
        let v = EmmetXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = EmmetXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn emmet_x_validator_validate_all() {
        let v = EmmetXValidator::new();
        let mut reg = EmmetXRegistry::new();
        reg.insert(EmmetXConfig::new("ok")).unwrap();
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


    // ---- xc_ pool / scheduler tests – block 42 ----

    #[test]
    fn xc_42_pool_new_empty() {
        let pool: super::Xc42Pool<i32> = super::Xc42Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_42_pool_release_acquire() {
        let mut pool = super::Xc42Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_42_pool_acquire_empty() {
        let mut pool: super::Xc42Pool<i32> = super::Xc42Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_42_pool_full() {
        let mut pool = super::Xc42Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_42_pool_drain() {
        let mut pool = super::Xc42Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_42_pool_stats() {
        let mut pool = super::Xc42Pool::new(8);
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
    fn xc_42_pool_clear() {
        let mut pool = super::Xc42Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_42_pool_shrink() {
        let mut pool = super::Xc42Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_42_pool_default() {
        let pool: super::Xc42Pool<String> = super::Xc42Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_42_pool_extend() {
        let mut pool = super::Xc42Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_42_pool_retain() {
        let mut pool = super::Xc42Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_42_scheduler_round_robin() {
        let mut sched = super::Xc42Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_42_scheduler_empty() {
        let mut sched = super::Xc42Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_42_scheduler_reset() {
        let mut sched = super::Xc42Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_42_scheduler_add_remove() {
        let mut sched = super::Xc42Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_42_scheduler_targets() {
        let sched = super::Xc42Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_42_hash_empty() {
        assert_eq!(super::xc_42_hash(b""), 5381);
    }

    #[test]
    fn xc_42_hash_data() {
        let h = super::xc_42_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_42_hash(b"hello"), h);
    }

    #[test]
    fn xc_42_reverse_str() {
        assert_eq!(super::xc_42_reverse("abc"), "cba");
        assert_eq!(super::xc_42_reverse(""), "");
    }


    // --- xd_11 deepening tests ---

    #[test]
    fn xd_11_sm_initial_state() {
        let sm = Xd11StateMachine::new();
        assert_eq!(sm.current_state(), Xd11State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_11_sm_valid_idle_to_running() {
        let mut sm = Xd11StateMachine::new();
        assert!(sm.transition(Xd11State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd11State::Running);
    }

    #[test]
    fn xd_11_sm_valid_running_to_paused() {
        let mut sm = Xd11StateMachine::new();
        sm.transition(Xd11State::Running).unwrap();
        assert!(sm.transition(Xd11State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd11State::Paused);
    }

    #[test]
    fn xd_11_sm_valid_running_to_done() {
        let mut sm = Xd11StateMachine::new();
        sm.transition(Xd11State::Running).unwrap();
        assert!(sm.transition(Xd11State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd11State::Done);
    }

    #[test]
    fn xd_11_sm_valid_paused_to_running() {
        let mut sm = Xd11StateMachine::new();
        sm.transition(Xd11State::Running).unwrap();
        sm.transition(Xd11State::Paused).unwrap();
        assert!(sm.transition(Xd11State::Running).is_ok());
    }

    #[test]
    fn xd_11_sm_valid_done_to_idle() {
        let mut sm = Xd11StateMachine::new();
        sm.transition(Xd11State::Running).unwrap();
        sm.transition(Xd11State::Done).unwrap();
        assert!(sm.transition(Xd11State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd11State::Idle);
    }

    #[test]
    fn xd_11_sm_invalid_idle_to_done() {
        let mut sm = Xd11StateMachine::new();
        assert!(sm.transition(Xd11State::Done).is_err());
    }

    #[test]
    fn xd_11_sm_invalid_idle_to_paused() {
        let mut sm = Xd11StateMachine::new();
        assert!(sm.transition(Xd11State::Paused).is_err());
    }

    #[test]
    fn xd_11_sm_history_tracking() {
        let mut sm = Xd11StateMachine::new();
        sm.transition(Xd11State::Running).unwrap();
        sm.transition(Xd11State::Paused).unwrap();
        sm.transition(Xd11State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd11State::Idle);
        assert_eq!(sm.history()[0].to, Xd11State::Running);
        assert_eq!(sm.history()[1].from, Xd11State::Running);
        assert_eq!(sm.history()[2].to, Xd11State::Done);
    }

    #[test]
    fn xd_11_sm_serialize_deserialize() {
        let mut sm = Xd11StateMachine::new();
        sm.transition(Xd11State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd11StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd11State::Running));
    }

    #[test]
    fn xd_11_sm_deserialize_invalid() {
        assert_eq!(Xd11StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_11_sm_reset() {
        let mut sm = Xd11StateMachine::new();
        sm.transition(Xd11State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd11State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_11_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd11EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd11Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_11_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd11EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd11Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd11Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_11_bus_unsubscribe() {
        let mut bus = Xd11EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_11_event_kind_and_payload() {
        let e = Xd11Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd11Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_11_bus_clear_history() {
        let mut bus = Xd11EventBus::new();
        bus.publish(Xd11Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_11_sm_step_counter_increments() {
        let mut sm = Xd11StateMachine::new();
        sm.transition(Xd11State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd11State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
