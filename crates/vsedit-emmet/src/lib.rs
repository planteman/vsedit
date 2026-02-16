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
}
