//! Hover tooltip service.
//!
//! Equivalent to VS Code's `vs/editor/contrib/hover`.
//! Provides hover content model for displaying tooltips at cursor positions.

use std::fmt;
/// How the hover was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverTriggerKind {
    /// Explicitly invoked (e.g. via keyboard shortcut).
    Invoke,
    /// Triggered by mouse hover.
    Hover,
    /// Triggered by content interaction (e.g. clicking a link).
    ContentHover,
}

/// Configuration for hover behaviour.
#[derive(Debug, Clone)]
pub struct HoverConfig {
    /// Whether hover is enabled.
    pub enabled: bool,
    /// Delay in milliseconds before showing hover.
    pub delay_ms: u32,
    /// Whether the hover stays visible when the mouse moves away.
    pub sticky: bool,
    /// Prefer showing the hover above the line.
    pub above_line_preference: bool,
}

impl Default for HoverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_ms: 300,
            sticky: true,
            above_line_preference: true,
        }
    }
}

/// Tracks the state of an active hover session.
#[derive(Debug, Clone)]
pub struct HoverSession {
    pub current_hover: Option<Hover>,
    pub line: u32,
    pub col: u32,
    pub visible: bool,
    pub pinned: bool,
}

impl HoverSession {
    pub fn new() -> Self {
        Self {
            current_hover: None,
            line: 0,
            col: 0,
            visible: false,
            pinned: false,
        }
    }

    /// Show a hover at the given position.
    pub fn show(&mut self, hover: Hover, line: u32, col: u32) {
        self.current_hover = Some(hover);
        self.line = line;
        self.col = col;
        self.visible = true;
    }

    /// Hide the current hover (unless pinned).
    pub fn hide(&mut self) {
        if !self.pinned {
            self.visible = false;
            self.current_hover = None;
        }
    }

    /// Toggle the pinned state. Pinned hovers remain visible until explicitly unpinned.
    pub fn toggle_pin(&mut self) {
        self.pinned = !self.pinned;
        if !self.pinned {
            self.visible = false;
            self.current_hover = None;
        }
    }
}

impl Default for HoverSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Content that can be displayed in a hover.
#[derive(Debug, Clone)]
pub enum HoverContent {
    /// Plain text.
    Text(String),
    /// Markdown text.
    Markdown(String),
    /// Code with optional language.
    Code {
        value: String,
        language: Option<String>,
    },
}

/// A hover result containing multiple content blocks.
#[derive(Debug, Clone)]
pub struct Hover {
    pub contents: Vec<HoverContent>,
    pub range: Option<HoverRange>,
}

/// The range a hover applies to.
#[derive(Debug, Clone, Copy)]
pub struct HoverRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl Hover {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            contents: vec![HoverContent::Text(text.into())],
            range: None,
        }
    }

    pub fn markdown(md: impl Into<String>) -> Self {
        Self {
            contents: vec![HoverContent::Markdown(md.into())],
            range: None,
        }
    }

    pub fn code(code: impl Into<String>, language: Option<&str>) -> Self {
        Self {
            contents: vec![HoverContent::Code {
                value: code.into(),
                language: language.map(|s| s.to_string()),
            }],
            range: None,
        }
    }

    /// Convenience constructor from a vec of contents.
    pub fn from_contents(contents: Vec<HoverContent>) -> Self {
        Self {
            contents,
            range: None,
        }
    }

    pub fn with_range(mut self, range: HoverRange) -> Self {
        self.range = Some(range);
        self
    }

    pub fn add_content(mut self, content: HoverContent) -> Self {
        self.contents.push(content);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    /// Number of content blocks.
    pub fn content_count(&self) -> usize {
        self.contents.len()
    }

    /// Returns true if any content block is a code block.
    pub fn has_code_content(&self) -> bool {
        self.contents
            .iter()
            .any(|c| matches!(c, HoverContent::Code { .. }))
    }
}

/// Provider for hover content.
pub trait HoverProvider: Send + Sync {
    fn provide_hover(&self, line: u32, column: u32) -> Option<Hover>;
}

/// Registry for hover providers.
pub struct HoverRegistry {
    providers: Vec<Box<dyn HoverProvider>>,
}

impl HoverRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn HoverProvider>) {
        self.providers.push(provider);
    }

    /// Number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Get combined hover content from all providers.
    pub fn provide_hover(&self, line: u32, column: u32) -> Option<Hover> {
        let mut contents = Vec::new();
        let mut range = None;

        for provider in &self.providers {
            if let Some(hover) = provider.provide_hover(line, column) {
                contents.extend(hover.contents);
                if range.is_none() {
                    range = hover.range;
                }
            }
        }

        if contents.is_empty() {
            None
        } else {
            Some(Hover { contents, range })
        }
    }
}

impl Default for HoverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Check whether a position falls within a hover range (inclusive).
pub fn is_position_in_range(range: &HoverRange, line: u32, col: u32) -> bool {
    if line < range.start_line || line > range.end_line {
        return false;
    }
    if line == range.start_line && col < range.start_column {
        return false;
    }
    if line == range.end_line && col > range.end_column {
        return false;
    }
    true
}

/// Merge multiple hover results into a single hover.
///
/// Contents are concatenated. The range of the first hover that has one is used.
pub fn merge_hovers(hovers: &[Hover]) -> Hover {
    let mut contents = Vec::new();
    let mut range = None;
    for hover in hovers {
        contents.extend(hover.contents.clone());
        if range.is_none() {
            range = hover.range;
        }
    }
    Hover { contents, range }
}

/// Render hover contents to a plain-text string.
pub fn render_hover_to_string(hover: &Hover) -> String {
    let mut out = String::new();
    for (i, content) in hover.contents.iter().enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str("---");
            out.push('\n');
        }
        match content {
            HoverContent::Text(t) => out.push_str(t),
            HoverContent::Markdown(md) => out.push_str(md),
            HoverContent::Code { value, language } => {
                if let Some(lang) = language {
                    out.push('[');
                    out.push_str(lang);
                    out.push_str("] ");
                }
                out.push_str(value);
            }
        }
    }
    out
}

/// Manages hover display timing based on trigger kind and configuration.
#[derive(Debug, Clone)]
pub struct HoverDelay;

impl HoverDelay {
    /// Returns true if enough time has elapsed to show the hover.
    pub fn should_show(elapsed_ms: u32, config: &HoverConfig) -> bool {
        config.enabled && elapsed_ms >= config.delay_ms
    }

    /// Computes the appropriate delay for a given trigger kind.
    /// Explicit invocations show immediately; mouse hovers use the configured delay.
    pub fn compute_delay(trigger_kind: HoverTriggerKind, config: &HoverConfig) -> u32 {
        match trigger_kind {
            HoverTriggerKind::Invoke => 0,
            HoverTriggerKind::Hover => config.delay_ms,
            HoverTriggerKind::ContentHover => config.delay_ms / 2,
        }
    }
}

/// Tracks positions where hovers have been shown, for frequency analysis.
#[derive(Debug, Clone)]
pub struct HoverHistory {
    entries: Vec<(u32, u32)>,
}

impl HoverHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record that a hover was shown at the given position.
    pub fn record(&mut self, line: u32, col: u32) {
        self.entries.push((line, col));
    }

    /// Return the top-N most frequently hovered positions as `(line, col, count)`.
    pub fn get_frequent_positions(&self, top_n: usize) -> Vec<(u32, u32, usize)> {
        use std::collections::HashMap;
        let mut counts: HashMap<(u32, u32), usize> = HashMap::new();
        for &pos in &self.entries {
            *counts.entry(pos).or_insert(0) += 1;
        }
        let mut sorted: Vec<(u32, u32, usize)> =
            counts.into_iter().map(|((l, c), n)| (l, c, n)).collect();
        sorted.sort_by(|a, b| b.2.cmp(&a.2));
        sorted.truncate(top_n);
        sorted
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for HoverHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Fluent builder for constructing [`Hover`] instances.
#[derive(Debug, Clone)]
pub struct HoverContentBuilder {
    contents: Vec<HoverContent>,
    range: Option<HoverRange>,
}

impl HoverContentBuilder {
    pub fn new() -> Self {
        Self {
            contents: Vec::new(),
            range: None,
        }
    }

    pub fn add_text(mut self, text: impl Into<String>) -> Self {
        self.contents.push(HoverContent::Text(text.into()));
        self
    }

    pub fn add_markdown(mut self, md: impl Into<String>) -> Self {
        self.contents.push(HoverContent::Markdown(md.into()));
        self
    }

    pub fn add_code(mut self, code: impl Into<String>, language: Option<&str>) -> Self {
        self.contents.push(HoverContent::Code {
            value: code.into(),
            language: language.map(|s| s.to_string()),
        });
        self
    }

    pub fn add_separator(mut self) -> Self {
        self.contents
            .push(HoverContent::Text("---".to_string()));
        self
    }

    pub fn set_range(mut self, range: HoverRange) -> Self {
        self.range = Some(range);
        self
    }

    pub fn build(self) -> Hover {
        Hover {
            contents: self.contents,
            range: self.range,
        }
    }
}

impl Default for HoverContentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns a new hover with each content block truncated to `max_chars`.
pub fn truncate_hover_content(hover: &Hover, max_chars: usize) -> Hover {
    let contents = hover
        .contents
        .iter()
        .map(|c| match c {
            HoverContent::Text(t) => {
                HoverContent::Text(t.chars().take(max_chars).collect())
            }
            HoverContent::Markdown(md) => {
                HoverContent::Markdown(md.chars().take(max_chars).collect())
            }
            HoverContent::Code { value, language } => HoverContent::Code {
                value: value.chars().take(max_chars).collect(),
                language: language.clone(),
            },
        })
        .collect();
    Hover {
        contents,
        range: hover.range,
    }
}

/// Returns the total character length across all content blocks in the hover.
pub fn hover_content_length(hover: &Hover) -> usize {
    hover
        .contents
        .iter()
        .map(|c| match c {
            HoverContent::Text(t) => t.len(),
            HoverContent::Markdown(md) => md.len(),
            HoverContent::Code { value, .. } => value.len(),
        })
        .sum()
}

/// Conditionally filters hover results by language and position constraints.
#[derive(Debug, Clone)]
pub struct HoverFilter {
    /// If set, only hovers that contain code blocks with this language are accepted.
    pub language: Option<String>,
    /// If set, only hovers whose range contains this position are accepted.
    pub position: Option<(u32, u32)>,
}

impl HoverFilter {
    pub fn new() -> Self {
        Self {
            language: None,
            position: None,
        }
    }

    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    pub fn with_position(mut self, line: u32, col: u32) -> Self {
        self.position = Some((line, col));
        self
    }

    /// Returns `true` if the hover passes all configured filters.
    pub fn accepts(&self, hover: &Hover) -> bool {
        if let Some(ref lang) = self.language {
            let has_lang = hover.contents.iter().any(|c| match c {
                HoverContent::Code { language, .. } => {
                    language.as_deref() == Some(lang.as_str())
                }
                _ => false,
            });
            if !has_lang {
                return false;
            }
        }
        if let Some((line, col)) = self.position {
            if let Some(ref range) = hover.range {
                if !is_position_in_range(range, line, col) {
                    return false;
                }
            }
        }
        true
    }
}

impl Default for HoverFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MarkdownString — terminal-renderable styled text
// ---------------------------------------------------------------------------

/// A styled text span for terminal rendering of markdown content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyledSpan {
    Plain(String),
    Bold(String),
    Italic(String),
    BoldItalic(String),
    InlineCode(String),
    CodeBlock { code: String, language: Option<String> },
    Link { text: String, url: String },
    Heading { level: u8, text: String },
    ListItem(String),
    Separator,
}

/// A line of styled spans ready for terminal rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledLine {
    pub spans: Vec<StyledSpan>,
}

impl StyledLine {
    pub fn new() -> Self {
        Self { spans: Vec::new() }
    }

    pub fn push(&mut self, span: StyledSpan) {
        self.spans.push(span);
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Compute the approximate display width of this line.
    pub fn display_width(&self) -> usize {
        self.spans.iter().map(|s| match s {
            StyledSpan::Plain(t)
            | StyledSpan::Bold(t)
            | StyledSpan::Italic(t)
            | StyledSpan::BoldItalic(t)
            | StyledSpan::InlineCode(t)
            | StyledSpan::ListItem(t) => t.len(),
            StyledSpan::CodeBlock { code, .. } => code.lines().map(|l| l.len()).max().unwrap_or(0),
            StyledSpan::Link { text, url } => text.len() + url.len() + 3,
            StyledSpan::Heading { text, .. } => text.len(),
            StyledSpan::Separator => 3,
        }).sum()
    }
}

impl Default for StyledLine {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse markdown text into styled lines for terminal rendering.
///
/// Handles: `**bold**`, `*italic*`, `` `code` ``, `[text](url)`, headings,
/// list items, fenced code blocks, and separators (`---`).
pub fn parse_markdown_to_styled(markdown: &str) -> Vec<StyledLine> {
    let mut lines = Vec::new();
    let src_lines: Vec<&str> = markdown.lines().collect();
    let len = src_lines.len();
    let mut i = 0;

    while i < len {
        let line = src_lines[i];

        // Fenced code block
        if line.trim_start().starts_with("```") {
            let lang = line.trim_start().trim_start_matches('`').trim();
            let language = if lang.is_empty() { None } else { Some(lang.to_string()) };
            let mut code = String::new();
            i += 1;
            while i < len && !src_lines[i].trim_start().starts_with("```") {
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(src_lines[i]);
                i += 1;
            }
            let mut sl = StyledLine::new();
            sl.push(StyledSpan::CodeBlock { code, language });
            lines.push(sl);
            i += 1;
            continue;
        }

        // Separator
        if line.trim() == "---" || line.trim() == "***" || line.trim() == "___" {
            let mut sl = StyledLine::new();
            sl.push(StyledSpan::Separator);
            lines.push(sl);
            i += 1;
            continue;
        }

        // Heading
        if line.starts_with('#') {
            let level = line.chars().take_while(|&c| c == '#').count().min(6) as u8;
            let text = line[level as usize..].trim().to_string();
            let mut sl = StyledLine::new();
            sl.push(StyledSpan::Heading { level, text });
            lines.push(sl);
            i += 1;
            continue;
        }

        // List item
        let trimmed = line.trim_start();
        if (trimmed.starts_with("- ") || trimmed.starts_with("* ")) && trimmed.len() > 2 {
            let content = trimmed[2..].to_string();
            let mut sl = StyledLine::new();
            sl.push(StyledSpan::ListItem(content));
            lines.push(sl);
            i += 1;
            continue;
        }

        // Inline formatting
        lines.push(parse_inline_styled(line));
        i += 1;
    }

    lines
}

/// Parse inline markdown formatting into styled spans.
fn parse_inline_styled(text: &str) -> StyledLine {
    let mut line = StyledLine::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut buf = String::new();

    while i < len {
        // Bold: **...**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_double_delim(&chars, i + 2, '*') {
                if !buf.is_empty() {
                    line.push(StyledSpan::Plain(std::mem::take(&mut buf)));
                }
                let content: String = chars[i + 2..end].iter().collect();
                line.push(StyledSpan::Bold(content));
                i = end + 2;
                continue;
            }
        }

        // Italic: *...*
        if chars[i] == '*' {
            if let Some(end) = find_single_delim(&chars, i + 1, '*') {
                if !buf.is_empty() {
                    line.push(StyledSpan::Plain(std::mem::take(&mut buf)));
                }
                let content: String = chars[i + 1..end].iter().collect();
                line.push(StyledSpan::Italic(content));
                i = end + 1;
                continue;
            }
        }

        // Inline code: `...`
        if chars[i] == '`' {
            if let Some(end) = find_single_delim(&chars, i + 1, '`') {
                if !buf.is_empty() {
                    line.push(StyledSpan::Plain(std::mem::take(&mut buf)));
                }
                let content: String = chars[i + 1..end].iter().collect();
                line.push(StyledSpan::InlineCode(content));
                i = end + 1;
                continue;
            }
        }

        // Link: [text](url)
        if chars[i] == '[' {
            if let Some(cb) = find_single_delim(&chars, i + 1, ']') {
                if cb + 1 < len && chars[cb + 1] == '(' {
                    if let Some(cp) = find_single_delim(&chars, cb + 2, ')') {
                        if !buf.is_empty() {
                            line.push(StyledSpan::Plain(std::mem::take(&mut buf)));
                        }
                        let text: String = chars[i + 1..cb].iter().collect();
                        let url: String = chars[cb + 2..cp].iter().collect();
                        line.push(StyledSpan::Link { text, url });
                        i = cp + 1;
                        continue;
                    }
                }
            }
        }

        buf.push(chars[i]);
        i += 1;
    }

    if !buf.is_empty() {
        line.push(StyledSpan::Plain(buf));
    }
    line
}

fn find_double_delim(chars: &[char], from: usize, delim: char) -> Option<usize> {
    let len = chars.len();
    let mut j = from;
    while j + 1 < len {
        if chars[j] == delim && chars[j + 1] == delim {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn find_single_delim(chars: &[char], from: usize, delim: char) -> Option<usize> {
    chars.iter().enumerate().skip(from).find_map(|(j, &c)| if c == delim { Some(j) } else { None })
}

/// Render hover contents to styled lines for terminal display.
///
/// Each content block is rendered into styled lines, separated by `---`.
pub fn render_hover_styled(hover: &Hover) -> Vec<StyledLine> {
    let mut result = Vec::new();
    for (i, content) in hover.contents.iter().enumerate() {
        if i > 0 {
            let mut sep = StyledLine::new();
            sep.push(StyledSpan::Separator);
            result.push(sep);
        }
        match content {
            HoverContent::Text(t) => {
                for line in t.lines() {
                    let mut sl = StyledLine::new();
                    sl.push(StyledSpan::Plain(line.to_string()));
                    result.push(sl);
                }
            }
            HoverContent::Markdown(md) => {
                result.extend(parse_markdown_to_styled(md));
            }
            HoverContent::Code { value, language } => {
                let mut sl = StyledLine::new();
                sl.push(StyledSpan::CodeBlock {
                    code: value.clone(),
                    language: language.clone(),
                });
                result.push(sl);
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// HoverWidget — floating overlay dimensions and layout
// ---------------------------------------------------------------------------

/// Computed layout for a hover tooltip overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoverWidget {
    /// X position (column) of the overlay origin.
    pub x: u16,
    /// Y position (row) of the overlay origin.
    pub y: u16,
    /// Width of the overlay in columns.
    pub width: u16,
    /// Height of the overlay in rows.
    pub height: u16,
}

impl HoverWidget {
    /// Compute overlay dimensions from styled lines, anchoring at the cursor.
    ///
    /// `cursor_x` / `cursor_y` are the cursor position in the terminal.
    /// `max_width` / `max_height` are the available terminal area.
    /// `prefer_above` positions the overlay above the cursor when possible.
    pub fn compute(
        styled_lines: &[StyledLine],
        cursor_x: u16,
        cursor_y: u16,
        max_width: u16,
        max_height: u16,
        prefer_above: bool,
    ) -> Self {
        let content_width = styled_lines
            .iter()
            .map(|l| l.display_width() as u16)
            .max()
            .unwrap_or(0)
            .min(max_width.saturating_sub(2)) // leave border room
            .max(10);
        let content_height = (styled_lines.len() as u16)
            .min(max_height.saturating_sub(2))
            .max(1);

        let width = content_width + 2; // 1-char padding each side
        let height = content_height + 2;

        let x = if cursor_x + width <= max_width {
            cursor_x
        } else {
            max_width.saturating_sub(width)
        };

        let y = if prefer_above && cursor_y >= height + 1 {
            cursor_y - height - 1
        } else if cursor_y + 2 + height <= max_height {
            cursor_y + 1
        } else {
            cursor_y.saturating_sub(height + 1)
        };

        Self { x, y, width, height }
    }

    /// Returns the area rectangle as `(x, y, width, height)`.
    pub fn area(&self) -> (u16, u16, u16, u16) {
        (self.x, self.y, self.width, self.height)
    }
}

/// Render hover result into a plain-text representation suitable for a
/// terminal overlay.  Returns lines of text.
pub fn render_hover(hover: &Hover, max_width: u16) -> Vec<String> {
    let styled = render_hover_styled(hover);
    let max_w = max_width as usize;
    let mut output = Vec::new();

    for sl in &styled {
        let mut line_text = String::new();
        for span in &sl.spans {
            match span {
                StyledSpan::Plain(t) => line_text.push_str(t),
                StyledSpan::Bold(t) => line_text.push_str(t),
                StyledSpan::Italic(t) => line_text.push_str(t),
                StyledSpan::BoldItalic(t) => line_text.push_str(t),
                StyledSpan::InlineCode(t) => {
                    line_text.push('`');
                    line_text.push_str(t);
                    line_text.push('`');
                }
                StyledSpan::CodeBlock { code, .. } => {
                    for code_line in code.lines() {
                        line_text.push_str(code_line);
                    }
                }
                StyledSpan::Link { text, url } => {
                    line_text.push_str(text);
                    line_text.push_str(" (");
                    line_text.push_str(url);
                    line_text.push(')');
                }
                StyledSpan::Heading { text, .. } => line_text.push_str(text),
                StyledSpan::ListItem(t) => {
                    line_text.push_str("• ");
                    line_text.push_str(t);
                }
                StyledSpan::Separator => line_text.push_str("───"),
            }
        }
        // Word-wrap long lines
        if line_text.len() > max_w && max_w > 0 {
            let mut remaining = line_text.as_str();
            while remaining.len() > max_w {
                output.push(remaining[..max_w].to_string());
                remaining = &remaining[max_w..];
            }
            if !remaining.is_empty() {
                output.push(remaining.to_string());
            }
        } else {
            output.push(line_text);
        }
    }

    output
}

// ---------------------------------------------------------------------------
// LSP hover parsing
// ---------------------------------------------------------------------------

/// Parse an LSP hover response into displayable content.
///
/// Handles `MarkupContent` (`{ kind, value }`), `MarkedString` (plain string),
/// and `MarkedString[]` (array of strings) response shapes.
pub fn parse_lsp_hover(response: &serde_json::Value) -> Option<HoverContent> {
    let contents = response.get("contents")?;
    if let Some(value) = contents.get("value").and_then(|v| v.as_str()) {
        let kind = contents
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("plaintext");
        Some(if kind == "markdown" {
            HoverContent::Markdown(value.to_string())
        } else {
            HoverContent::Text(value.to_string())
        })
    } else if let Some(text) = contents.as_str() {
        Some(HoverContent::Text(text.to_string()))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// HoverProviderRegistry – per-language provider management
// ---------------------------------------------------------------------------

/// Registry that associates hover providers with specific language IDs.
pub struct HoverProviderRegistry {
    entries: Vec<(String, Box<dyn HoverProvider>)>,
}

impl HoverProviderRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Register a provider for a specific language.
    pub fn register(&mut self, language_id: impl Into<String>, provider: Box<dyn HoverProvider>) {
        self.entries.push((language_id.into(), provider));
    }

    /// Unregister all providers for a given language. Returns count removed.
    pub fn unregister(&mut self, language_id: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(lang, _)| lang != language_id);
        before - self.entries.len()
    }

    pub fn provider_count(&self) -> usize {
        self.entries.len()
    }

    /// Get all providers registered for a given language.
    pub fn providers_for_language(&self, language_id: &str) -> Vec<&dyn HoverProvider> {
        self.entries.iter()
            .filter(|(lang, _)| lang == language_id)
            .map(|(_, p)| p.as_ref())
            .collect()
    }

    /// List all registered language IDs (deduplicated).
    pub fn languages(&self) -> Vec<&str> {
        let mut langs: Vec<&str> = self.entries.iter().map(|(l, _)| l.as_str()).collect();
        langs.sort();
        langs.dedup();
        langs
    }
}

// ---------------------------------------------------------------------------
// HoverCache – simple memoization for hover results
// ---------------------------------------------------------------------------

/// Caches hover results keyed by (line, column).
pub struct HoverCache {
    entries: Vec<((u32, u32), Hover)>,
    capacity: usize,
}

impl HoverCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::with_capacity(capacity), capacity }
    }

    pub fn get(&self, line: u32, column: u32) -> Option<&Hover> {
        self.entries.iter()
            .find(|((l, c), _)| *l == line && *c == column)
            .map(|(_, h)| h)
    }

    pub fn put(&mut self, line: u32, column: u32, hover: Hover) {
        self.entries.retain(|((l, c), _)| !(*l == line && *c == column));
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(((line, column), hover));
    }

    pub fn invalidate(&mut self, line: u32, column: u32) -> bool {
        let before = self.entries.len();
        self.entries.retain(|((l, c), _)| !(*l == line && *c == column));
        self.entries.len() < before
    }

    /// Invalidate all entries on a given line.
    pub fn invalidate_line(&mut self, line: u32) {
        self.entries.retain(|((l, _), _)| *l != line);
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
}

// ── Hover utilities ─────────────────────────────────────────────────────

/// Extract plain text from hover contents, discarding formatting.
pub fn extract_plain_text(hover: &Hover) -> String {
    hover
        .contents
        .iter()
        .map(|c| match c {
            HoverContent::Text(t) => t.as_str(),
            HoverContent::Markdown(m) => m.as_str(),
            HoverContent::Code { value, .. } => value.as_str(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Count the total number of characters across all hover contents.
pub fn hover_char_count(hover: &Hover) -> usize {
    hover.contents.iter().map(|c| match c {
        HoverContent::Text(t) => t.len(),
        HoverContent::Markdown(m) => m.len(),
        HoverContent::Code { value, .. } => value.len(),
    }).sum()
}

/// Filter hover contents to only code blocks.
pub fn extract_code_blocks(hover: &Hover) -> Vec<&HoverContent> {
    hover
        .contents
        .iter()
        .filter(|c| matches!(c, HoverContent::Code { .. }))
        .collect()
}

/// Create a hover that wraps each content item with a separator between them.
pub fn hover_with_separators(hovers: &[Hover], separator: &str) -> Hover {
    let mut contents = Vec::new();
    for (i, h) in hovers.iter().enumerate() {
        if i > 0 && !separator.is_empty() {
            contents.push(HoverContent::Text(separator.to_string()));
        }
        contents.extend(h.contents.iter().cloned());
    }
    Hover { contents, range: hovers.first().and_then(|h| h.range) }
}

/// Check if a hover contains any markdown content.
pub fn has_markdown(hover: &Hover) -> bool {
    hover.contents.iter().any(|c| matches!(c, HoverContent::Markdown(_)))
}

/// Compute the total line count of all hover content when rendered.
pub fn hover_line_count(hover: &Hover) -> usize {
    hover.contents.iter().map(|c| {
        let text = match c {
            HoverContent::Text(t) => t.as_str(),
            HoverContent::Markdown(m) => m.as_str(),
            HoverContent::Code { value, .. } => value.as_str(),
        };
        text.lines().count().max(1)
    }).sum()
}

/// Create a truncated version of a hover, limiting to `max_items` content blocks.
pub fn truncate_hover(hover: &Hover, max_items: usize) -> Hover {
    let contents: Vec<HoverContent> = hover.contents.iter().take(max_items).cloned().collect();
    Hover { contents, range: hover.range }
}

/// Check if a hover contains any plain text content.
pub fn has_text(hover: &Hover) -> bool {
    hover.contents.iter().any(|c| matches!(c, HoverContent::Text(_)))
}

/// Check if a hover contains any code content.
pub fn has_code(hover: &Hover) -> bool {
    hover.contents.iter().any(|c| matches!(c, HoverContent::Code { .. }))
}

/// Extract all text content from a hover, ignoring code and markdown blocks.
pub fn extract_text_content(hover: &Hover) -> Vec<&str> {
    hover
        .contents
        .iter()
        .filter_map(|c| match c {
            HoverContent::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

/// Extract all languages referenced in code blocks.
pub fn extract_languages(hover: &Hover) -> Vec<&str> {
    hover
        .contents
        .iter()
        .filter_map(|c| match c {
            HoverContent::Code { language, .. } => language.as_deref(),
            _ => None,
        })
        .collect()
}

impl HoverSession {
    /// Return true if the session has a hover at the given position.
    pub fn is_at(&self, line: u32, col: u32) -> bool {
        self.visible && self.line == line && self.col == col
    }

    /// Unpin the session if pinned, hiding the hover.
    pub fn unpin(&mut self) {
        if self.pinned {
            self.pinned = false;
            self.visible = false;
            self.current_hover = None;
        }
    }

    /// Return true if there is currently a hover shown (visible and non-empty).
    pub fn has_content(&self) -> bool {
        self.visible && self.current_hover.is_some()
    }
}

impl HoverRange {
    /// Create a new hover range.
    pub fn new(start_line: u32, start_column: u32, end_line: u32, end_column: u32) -> Self {
        Self {
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }

    /// Return the number of lines this range spans (inclusive).
    pub fn line_span(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Return true if this is a single-line range.
    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }

    /// Return true if the given position is within this range.
    pub fn contains(&self, line: u32, col: u32) -> bool {
        is_position_in_range(self, line, col)
    }
}

impl HoverHistory {
    /// Return the number of unique positions that have been hovered.
    pub fn unique_positions(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        for &pos in &self.entries {
            seen.insert(pos);
        }
        seen.len()
    }

    /// Return true if a specific position was ever hovered.
    pub fn was_hovered(&self, line: u32, col: u32) -> bool {
        self.entries.iter().any(|&(l, c)| l == line && c == col)
    }
}

impl HoverConfig {
    /// Return a config with hover disabled.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Return a config with sticky mode off.
    pub fn non_sticky() -> Self {
        Self {
            sticky: false,
            ..Self::default()
        }
    }
}

// ---------------------------------------------------------------------------
// HoverContentRenderer – renders hover content to string representations
// ---------------------------------------------------------------------------

/// Renders `Hover` content blocks into displayable strings.
pub struct HoverContentRenderer;

impl HoverContentRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Render the first content block of a hover to a string.
    pub fn render_to_string(hover: &Hover) -> String {
        match hover.contents.first() {
            Some(HoverContent::Markdown(md)) => md.clone(),
            Some(HoverContent::Text(t)) => format!("[text] {t}"),
            Some(HoverContent::Code { value, language }) => {
                let lang = language.as_deref().unwrap_or("");
                format!("```{lang}\n{value}\n```")
            }
            None => String::new(),
        }
    }

    /// Render the first content block, truncating to `max_len` characters.
    pub fn render_truncated(hover: &Hover, max_len: usize) -> String {
        let full = Self::render_to_string(hover);
        if full.len() <= max_len {
            full
        } else {
            let mut s = full[..max_len].to_string();
            s.push_str("...");
            s
        }
    }

    /// Count the words in the first content block.
    pub fn word_count(hover: &Hover) -> usize {
        let text = match hover.contents.first() {
            Some(HoverContent::Markdown(md)) => md.as_str(),
            Some(HoverContent::Text(t)) => t.as_str(),
            Some(HoverContent::Code { value, .. }) => value.as_str(),
            None => return 0,
        };
        text.split_whitespace().count()
    }

    /// Return a static label describing the content type.
    pub fn content_type_label(hover: &Hover) -> &'static str {
        match hover.contents.first() {
            Some(HoverContent::Markdown(_)) => "markdown",
            Some(HoverContent::Text(_)) => "plaintext",
            Some(HoverContent::Code { .. }) => "code",
            None => "empty",
        }
    }
}

impl Default for HoverContentRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// VerbosityMode / HoverVerbosity – compact vs expanded display
// ---------------------------------------------------------------------------

/// Whether hover content is displayed in compact or expanded form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbosityMode {
    Compact,
    Expanded,
}

impl std::fmt::Display for VerbosityMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerbosityMode::Compact => write!(f, "compact"),
            VerbosityMode::Expanded => write!(f, "expanded"),
        }
    }
}

/// Controls how many lines of hover content are shown.
#[derive(Debug, Clone)]
pub struct HoverVerbosity {
    pub mode: VerbosityMode,
    pub compact_max_lines: usize,
    pub expanded_max_lines: usize,
}

impl HoverVerbosity {
    pub fn new() -> Self {
        Self {
            mode: VerbosityMode::Compact,
            compact_max_lines: 5,
            expanded_max_lines: 50,
        }
    }

    /// Toggle between compact and expanded mode.
    pub fn toggle(&mut self) {
        self.mode = match self.mode {
            VerbosityMode::Compact => VerbosityMode::Expanded,
            VerbosityMode::Expanded => VerbosityMode::Compact,
        };
    }

    /// Maximum number of lines for the current mode.
    pub fn current_max_lines(&self) -> usize {
        match self.mode {
            VerbosityMode::Compact => self.compact_max_lines,
            VerbosityMode::Expanded => self.expanded_max_lines,
        }
    }

    /// Truncate content to the current mode's line limit.
    pub fn truncate_content(&self, content: &str) -> String {
        let max = self.current_max_lines();
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() <= max {
            content.to_string()
        } else {
            let mut out: String = lines[..max].join("\n");
            out.push_str("\n...");
            out
        }
    }

    pub fn is_compact(&self) -> bool {
        self.mode == VerbosityMode::Compact
    }
}

impl Default for HoverVerbosity {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HoverCodeBlock – a parsed code block with metadata
// ---------------------------------------------------------------------------

/// A single code block extracted from hover content.
#[derive(Debug, Clone)]
pub struct HoverCodeBlock {
    pub language: String,
    pub code: String,
    pub line_count: usize,
}

impl HoverCodeBlock {
    pub fn new(language: &str, code: &str) -> Self {
        let line_count = code.lines().count().max(1);
        Self {
            language: language.to_string(),
            code: code.to_string(),
            line_count,
        }
    }

    /// Return the individual lines of the code block.
    pub fn lines(&self) -> Vec<&str> {
        self.code.lines().collect()
    }

    pub fn is_single_line(&self) -> bool {
        self.line_count == 1
    }

    /// Render the code with 1-based line numbers prepended.
    pub fn render_with_line_numbers(&self) -> String {
        let width = self.line_count.to_string().len();
        self.code
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{:>width$} | {line}", i + 1, width = width))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn language_label(&self) -> &str {
        &self.language
    }
}

// ---------------------------------------------------------------------------
// HoverLinkHandler – extract and categorise links from hover content
// ---------------------------------------------------------------------------

/// The kind of link found in hover content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoverLinkType {
    Url,
    FileReference,
    Definition,
}

impl std::fmt::Display for HoverLinkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HoverLinkType::Url => write!(f, "url"),
            HoverLinkType::FileReference => write!(f, "file"),
            HoverLinkType::Definition => write!(f, "definition"),
        }
    }
}

/// A single link found inside hover content.
#[derive(Debug, Clone)]
pub struct HoverLink {
    pub url: String,
    pub label: Option<String>,
    pub link_type: HoverLinkType,
}

/// Extracts and stores links found in hover content.
#[derive(Debug, Clone)]
pub struct HoverLinkHandler {
    pub links: Vec<HoverLink>,
}

impl HoverLinkHandler {
    pub fn new() -> Self {
        Self { links: Vec::new() }
    }

    /// Scan `content` for markdown links `[label](url)` and bare `https://` URLs.
    pub fn extract_links(content: &str) -> Vec<HoverLink> {
        let mut links = Vec::new();

        // Markdown-style links: [label](url)
        let mut rest = content;
        while let Some(open) = rest.find('[') {
            let after_open = &rest[open + 1..];
            if let Some(close) = after_open.find("](") {
                let label = &after_open[..close];
                let after_paren = &after_open[close + 2..];
                if let Some(end) = after_paren.find(')') {
                    let url = &after_paren[..end];
                    let link_type = Self::classify_url(url);
                    links.push(HoverLink {
                        url: url.to_string(),
                        label: Some(label.to_string()),
                        link_type,
                    });
                    rest = &after_paren[end + 1..];
                    continue;
                }
            }
            rest = &rest[open + 1..];
        }

        // Bare https:// URLs (only those not already captured)
        for word in content.split_whitespace() {
            if word.starts_with("https://") || word.starts_with("http://") {
                let url = word.trim_end_matches(|c: char| c == ')' || c == ',' || c == '.');
                let already = links.iter().any(|l| l.url == url);
                if !already {
                    links.push(HoverLink {
                        url: url.to_string(),
                        label: None,
                        link_type: HoverLinkType::Url,
                    });
                }
            }
        }

        links
    }

    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// Return only file-reference links.
    pub fn file_links(&self) -> Vec<&HoverLink> {
        self.links
            .iter()
            .filter(|l| l.link_type == HoverLinkType::FileReference)
            .collect()
    }

    fn classify_url(url: &str) -> HoverLinkType {
        if url.starts_with("file://") || url.ends_with(".rs") || url.contains('/') && !url.starts_with("http") {
            HoverLinkType::FileReference
        } else if url.starts_with('#') {
            HoverLinkType::Definition
        } else {
            HoverLinkType::Url
        }
    }
}

impl Default for HoverLinkHandler {
    fn default() -> Self {
        Self::new()
    }
}


// ── Hover Content Size Calculator ──

/// Estimated dimensions for rendered hover content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoverDimensions {
    pub width_chars: u32,
    pub height_lines: u32,
    pub estimated_pixel_width: f64,
    pub estimated_pixel_height: f64,
}

impl HoverDimensions {
    pub fn new(width: u32, height: u32, char_width: f64, line_height: f64) -> Self {
        Self {
            width_chars: width,
            height_lines: height,
            estimated_pixel_width: width as f64 * char_width,
            estimated_pixel_height: height as f64 * line_height,
        }
    }

    /// Check if the hover would exceed given pixel bounds.
    pub fn exceeds_bounds(&self, max_width: f64, max_height: f64) -> bool {
        self.estimated_pixel_width > max_width || self.estimated_pixel_height > max_height
    }
}

/// Estimates display dimensions for hover content.
pub struct HoverContentSizeCalculator {
    char_width_px: f64,
    line_height_px: f64,
    max_width_chars: u32,
    padding_lines: u32,
}

impl HoverContentSizeCalculator {
    pub fn new(char_width_px: f64, line_height_px: f64) -> Self {
        Self {
            char_width_px,
            line_height_px,
            max_width_chars: 80,
            padding_lines: 2,
        }
    }

    /// Set maximum width in characters before wrapping.
    pub fn with_max_width(mut self, max: u32) -> Self {
        self.max_width_chars = max;
        self
    }

    /// Set padding lines added above and below content.
    pub fn with_padding(mut self, padding: u32) -> Self {
        self.padding_lines = padding;
        self
    }

    /// Estimate dimensions for plain text content.
    pub fn estimate_text(&self, text: &str) -> HoverDimensions {
        if text.is_empty() {
            return HoverDimensions::new(0, self.padding_lines, self.char_width_px, self.line_height_px);
        }
        let mut max_line_width = 0u32;
        let mut total_lines = 0u32;
        for line in text.lines() {
            let line_len = line.len() as u32;
            if line_len <= self.max_width_chars {
                max_line_width = max_line_width.max(line_len);
                total_lines += 1;
            } else {
                // Line wraps
                max_line_width = self.max_width_chars;
                total_lines += (line_len + self.max_width_chars - 1) / self.max_width_chars;
            }
        }
        HoverDimensions::new(
            max_line_width,
            total_lines + self.padding_lines,
            self.char_width_px,
            self.line_height_px,
        )
    }

    /// Estimate dimensions for markdown content (approximation).
    pub fn estimate_markdown(&self, markdown: &str) -> HoverDimensions {
        let cleaned = Self::strip_markdown_syntax(markdown);
        let mut dims = self.estimate_text(&cleaned);
        // Code blocks add extra height for borders
        let code_block_count = markdown.matches("```").count() / 2;
        dims.height_lines += code_block_count as u32 * 2;
        dims
    }

    /// Strip basic markdown syntax for size estimation.
    fn strip_markdown_syntax(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        for line in text.lines() {
            let trimmed = line.trim_start_matches('#').trim_start_matches('>').trim_start();
            let trimmed = trimmed.trim_start_matches("- ").trim_start_matches("* ");
            result.push_str(trimmed);
            result.push('\n');
        }
        result
    }

    /// Estimate dimensions for a code block.
    pub fn estimate_code_block(&self, code: &str, _language: &str) -> HoverDimensions {
        let line_count = code.lines().count() as u32;
        let max_width = code.lines().map(|l| l.len() as u32).max().unwrap_or(0);
        HoverDimensions::new(
            max_width.min(self.max_width_chars),
            line_count + self.padding_lines + 2, // +2 for code block borders
            self.char_width_px,
            self.line_height_px,
        )
    }

    /// Estimate combined dimensions for multiple content blocks.
    pub fn estimate_combined(&self, blocks: &[&str]) -> HoverDimensions {
        let mut total_height = 0u32;
        let mut max_width = 0u32;
        for block in blocks {
            let dims = self.estimate_text(block);
            total_height += dims.height_lines;
            max_width = max_width.max(dims.width_chars);
        }
        HoverDimensions::new(max_width, total_height, self.char_width_px, self.line_height_px)
    }
}

// ── Hover Widget Animator ──

/// Animation phase for hover widget transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationPhase {
    Hidden,
    FadingIn,
    Visible,
    FadingOut,
}

impl std::fmt::Display for AnimationPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimationPhase::Hidden => write!(f, "hidden"),
            AnimationPhase::FadingIn => write!(f, "fading_in"),
            AnimationPhase::Visible => write!(f, "visible"),
            AnimationPhase::FadingOut => write!(f, "fading_out"),
        }
    }
}

/// Manages fade-in/out animation state for hover widgets.
pub struct HoverWidgetAnimator {
    phase: AnimationPhase,
    opacity: f64,
    fade_in_duration_ms: u64,
    fade_out_duration_ms: u64,
    elapsed_ms: u64,
}

impl HoverWidgetAnimator {
    pub fn new(fade_in_ms: u64, fade_out_ms: u64) -> Self {
        Self {
            phase: AnimationPhase::Hidden,
            opacity: 0.0,
            fade_in_duration_ms: fade_in_ms,
            fade_out_duration_ms: fade_out_ms,
            elapsed_ms: 0,
        }
    }

    /// Start the fade-in animation.
    pub fn start_fade_in(&mut self) {
        self.phase = AnimationPhase::FadingIn;
        self.elapsed_ms = 0;
    }

    /// Start the fade-out animation.
    pub fn start_fade_out(&mut self) {
        self.phase = AnimationPhase::FadingOut;
        self.elapsed_ms = 0;
    }

    /// Advance the animation by a given number of milliseconds.
    pub fn tick(&mut self, delta_ms: u64) {
        self.elapsed_ms += delta_ms;
        match self.phase {
            AnimationPhase::FadingIn => {
                if self.fade_in_duration_ms == 0 {
                    self.opacity = 1.0;
                    self.phase = AnimationPhase::Visible;
                } else {
                    self.opacity = (self.elapsed_ms as f64 / self.fade_in_duration_ms as f64).min(1.0);
                    if self.opacity >= 1.0 {
                        self.phase = AnimationPhase::Visible;
                    }
                }
            }
            AnimationPhase::FadingOut => {
                if self.fade_out_duration_ms == 0 {
                    self.opacity = 0.0;
                    self.phase = AnimationPhase::Hidden;
                } else {
                    self.opacity = 1.0 - (self.elapsed_ms as f64 / self.fade_out_duration_ms as f64).min(1.0);
                    if self.opacity <= 0.0 {
                        self.phase = AnimationPhase::Hidden;
                    }
                }
            }
            _ => {}
        }
    }

    /// Jump to fully visible immediately.
    pub fn show_immediate(&mut self) {
        self.phase = AnimationPhase::Visible;
        self.opacity = 1.0;
        self.elapsed_ms = 0;
    }

    /// Jump to hidden immediately.
    pub fn hide_immediate(&mut self) {
        self.phase = AnimationPhase::Hidden;
        self.opacity = 0.0;
        self.elapsed_ms = 0;
    }

    pub fn phase(&self) -> AnimationPhase {
        self.phase
    }

    pub fn opacity(&self) -> f64 {
        self.opacity
    }

    pub fn is_visible(&self) -> bool {
        self.opacity > 0.0
    }

    pub fn is_animating(&self) -> bool {
        matches!(self.phase, AnimationPhase::FadingIn | AnimationPhase::FadingOut)
    }
}



// ─── Hover LRU Cache ───────────────────────────────────────

/// A simple LRU cache for hover content.
#[derive(Debug)]
pub struct HoverLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> HoverLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for HoverLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HoverLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── Hover Formatter ───────────────────────────────────────

/// Formatting options for hover tooltip output.
#[derive(Debug, Clone)]
pub struct HoverFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for HoverFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl HoverFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for hover tooltip data.
pub struct HoverFmt {
    options: HoverFmtOpts,
}

impl HoverFmt {
    pub fn new(options: HoverFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: HoverFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}



// ---------------------------------------------------------------------------
// hover – Extended hover delay helpers
// ---------------------------------------------------------------------------

/// Priority levels for hover delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZHoverPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZHoverPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZHoverPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZHoverPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks hover delay data.
#[derive(Debug, Clone)]
pub struct ZHoverHoverDelay {
    pub trigger_times_ms: Vec<u64>,
    pub base_delay_ms: u64,
    pub active: bool,
}

impl ZHoverHoverDelay {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            trigger_times_ms: Vec::new(),
            base_delay_ms: 0,
            active: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.trigger_times_ms.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.trigger_times_ms.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.trigger_times_ms.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZHoverHoverDelay[base_delay_ms={:?}, active={:?}]", self.base_delay_ms, self.active)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.active = !c.active;
        c
    }
}

/// Compute a simple rolling hash for hover delay.
pub fn z_hover_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_hover_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_hover_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_hover_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_hover_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_hover_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_hover_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
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
// xc_ pool and scheduler – generated block 90
// ---------------------------------------------------------------------------

/// Generic object pool `Xc90Pool<T>`.
pub struct Xc90Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc90Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc90PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc90Pool<T> {
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
    pub fn stats(&self) -> Xc90PoolStats {
        Xc90PoolStats {
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

impl<T> Default for Xc90Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc90Scheduler`.
pub struct Xc90Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc90Scheduler {
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

impl Default for Xc90Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_90 hash for the given byte slice.
pub fn xc_90_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_90 convention.
pub fn xc_90_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_4 deepening: state machine + event bus ---

/// States for the Xd4 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd4State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd4State {
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
pub struct Xd4Transition {
    pub from: Xd4State,
    pub to: Xd4State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd4StateMachine {
    current: Xd4State,
    history: Vec<Xd4Transition>,
    step_counter: usize,
}

impl Xd4StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd4State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd4State {
        self.current
    }

    pub fn history(&self) -> &[Xd4Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd4State) -> Result<Xd4State, String> {
        let allowed = match (self.current, target) {
            (Xd4State::Idle, Xd4State::Running) => true,
            (Xd4State::Running, Xd4State::Paused) => true,
            (Xd4State::Running, Xd4State::Done) => true,
            (Xd4State::Paused, Xd4State::Running) => true,
            (Xd4State::Paused, Xd4State::Done) => true,
            (Xd4State::Done, Xd4State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_4: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd4Transition {
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
            "Xd4SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd4State> {
        let prefix = "Xd4SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd4State::Idle),
            "Running" => Some(Xd4State::Running),
            "Paused" => Some(Xd4State::Paused),
            "Done" => Some(Xd4State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd4State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd4 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd4Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd4Event {
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

type Xd4HandlerFn = Box<dyn Fn(&Xd4Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd4EventBus {
    handlers: Vec<(usize, Option<String>, Xd4HandlerFn)>,
    next_id: usize,
    published: Vec<Xd4Event>,
}

impl Xd4EventBus {
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
        F: Fn(&Xd4Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd4Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd4Event) {
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

    pub fn published_events(&self) -> &[Xd4Event] {
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

    struct TestProvider;
    impl HoverProvider for TestProvider {
        fn provide_hover(&self, line: u32, _column: u32) -> Option<Hover> {
            if line == 1 {
                Some(Hover::markdown("**bold** text"))
            } else {
                None
            }
        }
    }

    #[test]
    fn hover_text() {
        let h = Hover::text("hello");
        assert!(!h.is_empty());
        assert!(matches!(&h.contents[0], HoverContent::Text(t) if t == "hello"));
    }

    #[test]
    fn hover_code() {
        let h = Hover::code("fn main() {}", Some("rust"));
        if let HoverContent::Code { value, language } = &h.contents[0] {
            assert_eq!(value, "fn main() {}");
            assert_eq!(language.as_deref(), Some("rust"));
        } else {
            panic!("Expected code");
        }
    }

    #[test]
    fn hover_registry() {
        let mut reg = HoverRegistry::new();
        reg.register(Box::new(TestProvider));

        assert!(reg.provide_hover(1, 1).is_some());
        assert!(reg.provide_hover(2, 1).is_none());
    }

    #[test]
    fn hover_with_range() {
        let h = Hover::text("info").with_range(HoverRange {
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 5,
        });
        assert!(h.range.is_some());
    }

    #[test]
    fn multi_content_hover() {
        let h = Hover::text("type: string")
            .add_content(HoverContent::Code {
                value: "let x: String".into(),
                language: Some("rust".into()),
            });
        assert_eq!(h.contents.len(), 2);
    }

    #[test]
    fn hover_config_defaults() {
        let cfg = HoverConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.delay_ms, 300);
        assert!(cfg.sticky);
        assert!(cfg.above_line_preference);
    }

    #[test]
    fn hover_session_show_and_hide() {
        let mut session = HoverSession::new();
        assert!(!session.visible);

        session.show(Hover::text("hello"), 5, 10);
        assert!(session.visible);
        assert_eq!(session.line, 5);
        assert_eq!(session.col, 10);
        assert!(session.current_hover.is_some());

        session.hide();
        assert!(!session.visible);
        assert!(session.current_hover.is_none());
    }

    #[test]
    fn hover_session_pin_prevents_hide() {
        let mut session = HoverSession::new();
        session.show(Hover::text("pinned"), 1, 1);
        session.toggle_pin();
        assert!(session.pinned);

        session.hide();
        assert!(session.visible);
        assert!(session.current_hover.is_some());
    }

    #[test]
    fn hover_session_unpin_hides() {
        let mut session = HoverSession::new();
        session.show(Hover::text("pinned"), 1, 1);
        session.toggle_pin();
        assert!(session.pinned);

        session.toggle_pin();
        assert!(!session.pinned);
        assert!(!session.visible);
        assert!(session.current_hover.is_none());
    }

    #[test]
    fn position_in_range_basic() {
        let range = HoverRange {
            start_line: 5,
            start_column: 3,
            end_line: 5,
            end_column: 10,
        };
        assert!(is_position_in_range(&range, 5, 3));
        assert!(is_position_in_range(&range, 5, 10));
        assert!(is_position_in_range(&range, 5, 7));
        assert!(!is_position_in_range(&range, 5, 2));
        assert!(!is_position_in_range(&range, 5, 11));
        assert!(!is_position_in_range(&range, 4, 5));
        assert!(!is_position_in_range(&range, 6, 5));
    }

    #[test]
    fn position_in_multiline_range() {
        let range = HoverRange {
            start_line: 2,
            start_column: 5,
            end_line: 4,
            end_column: 8,
        };
        assert!(!is_position_in_range(&range, 2, 4));
        assert!(is_position_in_range(&range, 2, 5));
        assert!(is_position_in_range(&range, 3, 0));
        assert!(is_position_in_range(&range, 3, 100));
        assert!(is_position_in_range(&range, 4, 8));
        assert!(!is_position_in_range(&range, 4, 9));
    }

    #[test]
    fn merge_hovers_combines_contents() {
        let h1 = Hover::text("first");
        let h2 = Hover::markdown("**second**");
        let merged = merge_hovers(&[h1, h2]);
        assert_eq!(merged.contents.len(), 2);
        assert!(merged.range.is_none());
    }

    #[test]
    fn merge_hovers_keeps_first_range() {
        let range = HoverRange {
            start_line: 1,
            start_column: 0,
            end_line: 1,
            end_column: 5,
        };
        let h1 = Hover::text("a");
        let h2 = Hover::text("b").with_range(range);
        let h3 = Hover::text("c").with_range(HoverRange {
            start_line: 9,
            start_column: 0,
            end_line: 9,
            end_column: 1,
        });
        let merged = merge_hovers(&[h1, h2, h3]);
        let r = merged.range.unwrap();
        assert_eq!(r.start_line, 1);
        assert_eq!(r.end_column, 5);
    }

    #[test]
    fn render_hover_plain_text() {
        let h = Hover::text("hello world");
        assert_eq!(render_hover_to_string(&h), "hello world");
    }

    #[test]
    fn render_hover_mixed() {
        let h = Hover::text("description")
            .add_content(HoverContent::Code {
                value: "fn foo()".into(),
                language: Some("rust".into()),
            });
        let rendered = render_hover_to_string(&h);
        assert!(rendered.contains("description"));
        assert!(rendered.contains("[rust] fn foo()"));
        assert!(rendered.contains("---"));
    }

    #[test]
    fn hover_from_contents() {
        let h = Hover::from_contents(vec![
            HoverContent::Text("one".into()),
            HoverContent::Markdown("**two**".into()),
        ]);
        assert_eq!(h.content_count(), 2);
        assert!(h.range.is_none());
    }

    #[test]
    fn hover_has_code_content() {
        let h1 = Hover::text("no code");
        assert!(!h1.has_code_content());

        let h2 = Hover::code("x = 1", Some("python"));
        assert!(h2.has_code_content());
    }

    #[test]
    fn hover_registry_provider_count() {
        let mut reg = HoverRegistry::new();
        assert_eq!(reg.provider_count(), 0);

        reg.register(Box::new(TestProvider));
        assert_eq!(reg.provider_count(), 1);
    }

    #[test]
    fn hover_trigger_kind_equality() {
        assert_eq!(HoverTriggerKind::Invoke, HoverTriggerKind::Invoke);
        assert_ne!(HoverTriggerKind::Hover, HoverTriggerKind::ContentHover);
    }

    #[test]
    fn hover_delay_should_show() {
        let cfg = HoverConfig::default();
        assert!(!HoverDelay::should_show(100, &cfg));
        assert!(HoverDelay::should_show(300, &cfg));
        assert!(HoverDelay::should_show(500, &cfg));

        let disabled = HoverConfig { enabled: false, ..HoverConfig::default() };
        assert!(!HoverDelay::should_show(1000, &disabled));
    }

    #[test]
    fn hover_delay_compute_delay() {
        let cfg = HoverConfig::default();
        assert_eq!(HoverDelay::compute_delay(HoverTriggerKind::Invoke, &cfg), 0);
        assert_eq!(HoverDelay::compute_delay(HoverTriggerKind::Hover, &cfg), 300);
        assert_eq!(HoverDelay::compute_delay(HoverTriggerKind::ContentHover, &cfg), 150);
    }

    #[test]
    fn hover_history_record_and_frequent() {
        let mut history = HoverHistory::new();
        assert!(history.is_empty());

        history.record(1, 5);
        history.record(1, 5);
        history.record(2, 3);
        history.record(1, 5);
        history.record(2, 3);
        assert_eq!(history.len(), 5);

        let top = history.get_frequent_positions(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0], (1, 5, 3));
        assert_eq!(top[1], (2, 3, 2));
    }

    #[test]
    fn hover_history_clear() {
        let mut history = HoverHistory::new();
        history.record(0, 0);
        history.record(1, 1);
        assert_eq!(history.len(), 2);
        history.clear();
        assert_eq!(history.len(), 0);
        assert!(history.is_empty());
    }

    #[test]
    fn hover_content_builder_fluent() {
        let hover = HoverContentBuilder::new()
            .add_text("Type info")
            .add_markdown("**bold**")
            .add_code("let x = 1;", Some("rust"))
            .add_separator()
            .set_range(HoverRange {
                start_line: 1,
                start_column: 0,
                end_line: 1,
                end_column: 10,
            })
            .build();
        assert_eq!(hover.content_count(), 4);
        assert!(hover.range.is_some());
        assert!(hover.has_code_content());
    }

    #[test]
    fn truncate_hover_content_test() {
        let hover = Hover::text("hello world");
        let truncated = truncate_hover_content(&hover, 5);
        assert_eq!(render_hover_to_string(&truncated), "hello");
    }

    #[test]
    fn hover_content_length_test() {
        let hover = Hover::text("abc")
            .add_content(HoverContent::Markdown("de".into()))
            .add_content(HoverContent::Code {
                value: "fgh".into(),
                language: None,
            });
        assert_eq!(hover_content_length(&hover), 8);
    }

    #[test]
    fn hover_filter_by_language() {
        let rust_hover = Hover::code("fn main() {}", Some("rust"));
        let py_hover = Hover::code("def main():", Some("python"));
        let text_hover = Hover::text("plain");

        let filter = HoverFilter::new().with_language("rust");
        assert!(filter.accepts(&rust_hover));
        assert!(!filter.accepts(&py_hover));
        assert!(!filter.accepts(&text_hover));
    }

    #[test]
    fn hover_filter_by_position() {
        let range = HoverRange {
            start_line: 5,
            start_column: 0,
            end_line: 5,
            end_column: 10,
        };
        let hover = Hover::text("info").with_range(range);

        let inside = HoverFilter::new().with_position(5, 5);
        assert!(inside.accepts(&hover));

        let outside = HoverFilter::new().with_position(6, 0);
        assert!(!outside.accepts(&hover));
    }

    #[test]
    fn hover_filter_no_constraints_accepts_all() {
        let filter = HoverFilter::new();
        assert!(filter.accepts(&Hover::text("anything")));
        assert!(filter.accepts(&Hover::code("x", Some("go"))));
    }

    // -----------------------------------------------------------------------
    // MarkdownString / styled rendering tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_inline_bold() {
        let lines = parse_markdown_to_styled("hello **world**");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans.contains(&StyledSpan::Bold("world".into())));
    }

    #[test]
    fn parse_inline_italic() {
        let lines = parse_markdown_to_styled("*emphasis*");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans.contains(&StyledSpan::Italic("emphasis".into())));
    }

    #[test]
    fn parse_inline_code() {
        let lines = parse_markdown_to_styled("use `cargo build`");
        assert!(lines[0].spans.contains(&StyledSpan::InlineCode("cargo build".into())));
    }

    #[test]
    fn parse_inline_link() {
        let lines = parse_markdown_to_styled("[docs](https://example.com)");
        assert!(lines[0].spans.contains(&StyledSpan::Link {
            text: "docs".into(),
            url: "https://example.com".into(),
        }));
    }

    #[test]
    fn parse_heading() {
        let lines = parse_markdown_to_styled("## API Reference");
        assert!(lines[0].spans.contains(&StyledSpan::Heading {
            level: 2,
            text: "API Reference".into(),
        }));
    }

    #[test]
    fn parse_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let lines = parse_markdown_to_styled(md);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans.contains(&StyledSpan::CodeBlock {
            code: "fn main() {}".into(),
            language: Some("rust".into()),
        }));
    }

    #[test]
    fn parse_list_item() {
        let lines = parse_markdown_to_styled("- item one");
        assert!(lines[0].spans.contains(&StyledSpan::ListItem("item one".into())));
    }

    #[test]
    fn parse_separator() {
        let lines = parse_markdown_to_styled("---");
        assert!(lines[0].spans.contains(&StyledSpan::Separator));
    }

    #[test]
    fn styled_line_display_width() {
        let mut sl = StyledLine::new();
        sl.push(StyledSpan::Plain("hello".into()));
        sl.push(StyledSpan::Bold("world".into()));
        assert_eq!(sl.display_width(), 10);
    }

    #[test]
    fn render_hover_styled_multiple_contents() {
        let hover = Hover::text("description")
            .add_content(HoverContent::Markdown("**bold**".into()));
        let styled = render_hover_styled(&hover);
        assert!(styled.len() >= 3); // text + separator + markdown
    }

    #[test]
    fn render_hover_wraps_long_lines() {
        let hover = Hover::text("a".repeat(100));
        let output = render_hover(&hover, 40);
        assert!(output.len() > 1);
        assert!(output[0].len() <= 40);
    }

    #[test]
    fn hover_widget_compute_basic() {
        let lines = vec![StyledLine::new()];
        let widget = HoverWidget::compute(&lines, 10, 5, 80, 24, true);
        assert!(widget.width > 0);
        assert!(widget.height > 0);
    }

    #[test]
    fn hover_widget_prefer_above() {
        let mut sl = StyledLine::new();
        sl.push(StyledSpan::Plain("test".into()));
        let lines = vec![sl];
        let widget = HoverWidget::compute(&lines, 10, 15, 80, 24, true);
        // Should position above cursor when there's room
        assert!(widget.y < 15);
    }

    #[test]
    fn hover_widget_area_tuple() {
        let w = HoverWidget { x: 5, y: 3, width: 20, height: 10 };
        assert_eq!(w.area(), (5, 3, 20, 10));
    }

    #[test]
    fn render_hover_code_content() {
        let hover = Hover::code("let x = 1;", Some("rust"));
        let output = render_hover(&hover, 80);
        assert!(!output.is_empty());
        assert!(output.iter().any(|l| l.contains("let x = 1;")));
    }

    // -----------------------------------------------------------------------
    // LSP hover parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_lsp_hover_markup_markdown() {
        let json = serde_json::json!({
            "contents": { "kind": "markdown", "value": "# Hello\n**bold**" }
        });
        let content = parse_lsp_hover(&json).unwrap();
        match content {
            HoverContent::Markdown(s) => assert!(s.contains("**bold**")),
            _ => panic!("expected Markdown variant"),
        }
    }

    #[test]
    fn parse_lsp_hover_markup_plaintext() {
        let json = serde_json::json!({
            "contents": { "kind": "plaintext", "value": "some info" }
        });
        let content = parse_lsp_hover(&json).unwrap();
        match content {
            HoverContent::Text(s) => assert_eq!(s, "some info"),
            _ => panic!("expected Text variant"),
        }
    }

    #[test]
    fn parse_lsp_hover_plain_string() {
        let json = serde_json::json!({ "contents": "just text" });
        let content = parse_lsp_hover(&json).unwrap();
        match content {
            HoverContent::Text(s) => assert_eq!(s, "just text"),
            _ => panic!("expected Text variant"),
        }
    }

    #[test]
    fn parse_lsp_hover_missing_contents() {
        assert!(parse_lsp_hover(&serde_json::json!({})).is_none());
        assert!(parse_lsp_hover(&serde_json::json!(null)).is_none());
    }

    #[test]
    fn parse_lsp_hover_empty_object_contents() {
        let json = serde_json::json!({ "contents": {} });
        assert!(parse_lsp_hover(&json).is_none());
    }

    #[test]
    fn parse_lsp_hover_no_kind_defaults_plaintext() {
        let json = serde_json::json!({
            "contents": { "value": "no kind field" }
        });
        let content = parse_lsp_hover(&json).unwrap();
        match content {
            HoverContent::Text(s) => assert_eq!(s, "no kind field"),
            _ => panic!("expected Text variant"),
        }
    }

    struct LangHoverProvider {
        lang: String,
    }
    impl HoverProvider for LangHoverProvider {
        fn provide_hover(&self, line: u32, _column: u32) -> Option<Hover> {
            if line == 10 {
                Some(Hover::text(format!("{} hover", self.lang)))
            } else {
                None
            }
        }
    }

    #[test]
    fn test_hover_provider_registry_register_and_lookup() {
        let mut reg = HoverProviderRegistry::new();
        reg.register("rust", Box::new(LangHoverProvider { lang: "rust".into() }));
        reg.register("python", Box::new(LangHoverProvider { lang: "python".into() }));
        assert_eq!(reg.provider_count(), 2);
        assert_eq!(reg.providers_for_language("rust").len(), 1);
        assert_eq!(reg.providers_for_language("go").len(), 0);
    }

    #[test]
    fn test_hover_provider_registry_unregister() {
        let mut reg = HoverProviderRegistry::new();
        reg.register("rust", Box::new(LangHoverProvider { lang: "rust".into() }));
        reg.register("rust", Box::new(LangHoverProvider { lang: "rust2".into() }));
        let removed = reg.unregister("rust");
        assert_eq!(removed, 2);
        assert_eq!(reg.provider_count(), 0);
    }

    #[test]
    fn test_hover_content_builder_full() {
        let hover = HoverContentBuilder::new()
            .add_text("hello")
            .add_code("let x = 1;", Some("rust"))
            .add_separator()
            .add_markdown("**bold**")
            .build();
        assert_eq!(hover.content_count(), 4);
        assert!(!hover.is_empty());
    }

    #[test]
    fn test_hover_content_builder_with_range() {
        let range = HoverRange { start_line: 1, start_column: 0, end_line: 1, end_column: 5 };
        let hover = HoverContentBuilder::new()
            .add_text("info")
            .set_range(range)
            .build();
        assert!(hover.range.is_some());
        assert_eq!(hover.contents.len(), 1);
    }

    #[test]
    fn test_hover_cache_put_get_invalidate() {
        let mut cache = HoverCache::new(10);
        cache.put(5, 3, Hover::text("hello"));
        assert_eq!(cache.len(), 1);
        assert!(cache.get(5, 3).is_some());
        assert!(cache.get(5, 4).is_none());
        cache.invalidate(5, 3);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_hover_cache_capacity() {
        let mut cache = HoverCache::new(2);
        cache.put(1, 0, Hover::text("a"));
        cache.put(2, 0, Hover::text("b"));
        cache.put(3, 0, Hover::text("c"));
        assert_eq!(cache.len(), 2);
        assert!(cache.get(1, 0).is_none());
        assert!(cache.get(2, 0).is_some());
        cache.invalidate_line(2);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn extract_plain_text_joins() {
        let hover = Hover::from_contents(vec![
            HoverContent::Text("hello".into()),
            HoverContent::Code { value: "world".into(), language: Some("rust".into()) },
        ]);
        let text = extract_plain_text(&hover);
        assert!(text.contains("hello"));
        assert!(text.contains("world"));
    }

    #[test]
    fn hover_char_count_sums() {
        let hover = Hover::from_contents(vec![
            HoverContent::Text("abc".into()),
            HoverContent::Markdown("de".into()),
        ]);
        assert_eq!(hover_char_count(&hover), 5);
        let empty = Hover::from_contents(vec![]);
        assert_eq!(hover_char_count(&empty), 0);
    }

    #[test]
    fn extract_code_blocks_filters() {
        let hover = Hover::from_contents(vec![
            HoverContent::Text("plain".into()),
            HoverContent::Code { value: "let x = 1;".into(), language: Some("rust".into()) },
            HoverContent::Markdown("**bold**".into()),
            HoverContent::Code { value: "print('hi')".into(), language: Some("python".into()) },
        ]);
        let blocks = extract_code_blocks(&hover);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn hover_with_separators_interleaves() {
        let h1 = Hover::text("first");
        let h2 = Hover::text("second");
        let combined = hover_with_separators(&[h1, h2], "---");
        assert_eq!(combined.contents.len(), 3);
    }

    #[test]
    fn hover_with_separators_empty() {
        let combined = hover_with_separators(&[], "---");
        assert!(combined.contents.is_empty());
        assert!(combined.range.is_none());
    }

    #[test]
    fn has_markdown_detection() {
        let md_hover = Hover::markdown("**bold**");
        assert!(has_markdown(&md_hover));
        let text_hover = Hover::text("plain");
        assert!(!has_markdown(&text_hover));
    }

    #[test]
    fn hover_line_count_counts() {
        let hover = Hover::from_contents(vec![
            HoverContent::Text("line1\nline2\nline3".into()),
            HoverContent::Text("single".into()),
        ]);
        assert_eq!(hover_line_count(&hover), 4);
    }

    #[test]
    fn truncate_hover_limits() {
        let hover = Hover::from_contents(vec![
            HoverContent::Text("a".into()),
            HoverContent::Text("b".into()),
            HoverContent::Text("c".into()),
        ]);
        let truncated = truncate_hover(&hover, 2);
        assert_eq!(truncated.contents.len(), 2);
        let full = truncate_hover(&hover, 10);
        assert_eq!(full.contents.len(), 3);
    }

    #[test]
    fn has_text_detection() {
        let hover = Hover::text("hello");
        assert!(has_text(&hover));
        let code_hover = Hover::code("fn main()", Some("rust"));
        assert!(!has_text(&code_hover));
    }

    #[test]
    fn has_code_detection() {
        let hover = Hover::code("fn main()", Some("rust"));
        assert!(has_code(&hover));
        let text_hover = Hover::text("hello");
        assert!(!has_code(&text_hover));
    }

    #[test]
    fn extract_text_content_filters() {
        let hover = Hover::from_contents(vec![
            HoverContent::Text("hello".into()),
            HoverContent::Markdown("**bold**".into()),
            HoverContent::Text("world".into()),
        ]);
        let texts = extract_text_content(&hover);
        assert_eq!(texts, vec!["hello", "world"]);
    }

    #[test]
    fn extract_languages_from_code() {
        let hover = Hover::from_contents(vec![
            HoverContent::Code { value: "x".into(), language: Some("rust".into()) },
            HoverContent::Code { value: "y".into(), language: Some("python".into()) },
            HoverContent::Text("z".into()),
        ]);
        let langs = extract_languages(&hover);
        assert_eq!(langs, vec!["rust", "python"]);
    }

    #[test]
    fn session_is_at_position() {
        let mut session = HoverSession::new();
        session.show(Hover::text("hi"), 5, 10);
        assert!(session.is_at(5, 10));
        assert!(!session.is_at(5, 11));
    }

    #[test]
    fn session_unpin() {
        let mut session = HoverSession::new();
        session.show(Hover::text("hi"), 1, 1);
        session.toggle_pin();
        assert!(session.pinned);
        session.unpin();
        assert!(!session.pinned);
        assert!(!session.visible);
    }

    #[test]
    fn session_has_content() {
        let mut session = HoverSession::new();
        assert!(!session.has_content());
        session.show(Hover::text("hi"), 1, 1);
        assert!(session.has_content());
    }

    #[test]
    fn hover_range_line_span() {
        let r = HoverRange::new(5, 0, 10, 20);
        assert_eq!(r.line_span(), 6);
        assert!(!r.is_single_line());
    }

    #[test]
    fn hover_range_single_line() {
        let r = HoverRange::new(5, 0, 5, 20);
        assert!(r.is_single_line());
        assert_eq!(r.line_span(), 1);
    }

    #[test]
    fn hover_range_contains() {
        let r = HoverRange::new(5, 0, 5, 20);
        assert!(r.contains(5, 10));
        assert!(!r.contains(6, 0));
    }

    #[test]
    fn history_unique_positions() {
        let mut h = HoverHistory::new();
        h.record(1, 1);
        h.record(1, 1);
        h.record(2, 2);
        assert_eq!(h.unique_positions(), 2);
    }

    #[test]
    fn history_was_hovered() {
        let mut h = HoverHistory::new();
        h.record(3, 7);
        assert!(h.was_hovered(3, 7));
        assert!(!h.was_hovered(3, 8));
    }

    #[test]
    fn config_disabled() {
        let cfg = HoverConfig::disabled();
        assert!(!cfg.enabled);
    }

    #[test]
    fn config_non_sticky() {
        let cfg = HoverConfig::non_sticky();
        assert!(!cfg.sticky);
        assert!(cfg.enabled);
    }

    // -- HoverContentRenderer tests --

    #[test]
    fn renderer_markdown_passthrough() {
        let h = Hover::markdown("**hello** world");
        assert_eq!(HoverContentRenderer::render_to_string(&h), "**hello** world");
        assert_eq!(HoverContentRenderer::content_type_label(&h), "markdown");
    }

    #[test]
    fn renderer_plain_text_wrapping() {
        let h = Hover::text("plain info");
        assert_eq!(HoverContentRenderer::render_to_string(&h), "[text] plain info");
        assert_eq!(HoverContentRenderer::content_type_label(&h), "plaintext");
    }

    #[test]
    fn renderer_code_block() {
        let h = Hover::code("fn main() {}", Some("rust"));
        let rendered = HoverContentRenderer::render_to_string(&h);
        assert!(rendered.starts_with("```rust\n"));
        assert!(rendered.ends_with("\n```"));
        assert_eq!(HoverContentRenderer::content_type_label(&h), "code");
    }

    #[test]
    fn renderer_truncation() {
        let h = Hover::markdown("abcdefghij");
        let truncated = HoverContentRenderer::render_truncated(&h, 5);
        assert_eq!(truncated, "abcde...");
        let not_truncated = HoverContentRenderer::render_truncated(&h, 100);
        assert_eq!(not_truncated, "abcdefghij");
    }

    #[test]
    fn renderer_word_count() {
        let h = Hover::markdown("one two three four");
        assert_eq!(HoverContentRenderer::word_count(&h), 4);
    }

    // -- HoverVerbosity tests --

    #[test]
    fn verbosity_defaults_compact() {
        let v = HoverVerbosity::new();
        assert!(v.is_compact());
        assert_eq!(v.current_max_lines(), 5);
    }

    #[test]
    fn verbosity_toggle() {
        let mut v = HoverVerbosity::new();
        v.toggle();
        assert!(!v.is_compact());
        assert_eq!(v.current_max_lines(), 50);
        assert_eq!(format!("{}", v.mode), "expanded");
        v.toggle();
        assert!(v.is_compact());
        assert_eq!(format!("{}", v.mode), "compact");
    }

    #[test]
    fn verbosity_truncate_content() {
        let v = HoverVerbosity {
            mode: VerbosityMode::Compact,
            compact_max_lines: 2,
            expanded_max_lines: 10,
        };
        let content = "line1\nline2\nline3\nline4";
        let truncated = v.truncate_content(content);
        assert_eq!(truncated, "line1\nline2\n...");
    }

    // -- HoverCodeBlock tests --

    #[test]
    fn code_block_basics() {
        let cb = HoverCodeBlock::new("rust", "let x = 1;\nlet y = 2;");
        assert_eq!(cb.line_count, 2);
        assert!(!cb.is_single_line());
        assert_eq!(cb.language_label(), "rust");
        assert_eq!(cb.lines(), vec!["let x = 1;", "let y = 2;"]);
    }

    #[test]
    fn code_block_line_numbers() {
        let cb = HoverCodeBlock::new("py", "a = 1\nb = 2\nc = 3");
        let numbered = cb.render_with_line_numbers();
        assert!(numbered.contains("1 | a = 1"));
        assert!(numbered.contains("3 | c = 3"));
    }

    #[test]
    fn code_block_single_line() {
        let cb = HoverCodeBlock::new("sh", "echo hi");
        assert!(cb.is_single_line());
    }

    // -- HoverLinkHandler tests --

    #[test]
    fn link_handler_extract_markdown_links() {
        let content = "See [docs](https://example.com) and [src](src/lib.rs).";
        let links = HoverLinkHandler::extract_links(content);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].url, "https://example.com");
        assert_eq!(links[0].label.as_deref(), Some("docs"));
        assert_eq!(links[0].link_type, HoverLinkType::Url);
        assert_eq!(links[1].url, "src/lib.rs");
        assert_eq!(links[1].link_type, HoverLinkType::FileReference);
    }

    #[test]
    fn link_handler_bare_urls() {
        let content = "Visit https://rust-lang.org for more.";
        let links = HoverLinkHandler::extract_links(content);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://rust-lang.org");
        assert!(links[0].label.is_none());
    }

    #[test]
    fn link_handler_file_links_filter() {
        let mut handler = HoverLinkHandler::new();
        handler.links.push(HoverLink {
            url: "https://example.com".into(),
            label: None,
            link_type: HoverLinkType::Url,
        });
        handler.links.push(HoverLink {
            url: "file://path/to/file.rs".into(),
            label: None,
            link_type: HoverLinkType::FileReference,
        });
        assert_eq!(handler.link_count(), 2);
        assert_eq!(handler.file_links().len(), 1);
    }

    #[test]
    fn link_type_display() {
        assert_eq!(format!("{}", HoverLinkType::Url), "url");
        assert_eq!(format!("{}", HoverLinkType::FileReference), "file");
        assert_eq!(format!("{}", HoverLinkType::Definition), "definition");
    }

    #[test]
    fn hover_dimensions_basic() {
        let d = HoverDimensions::new(40, 10, 8.0, 16.0);
        assert_eq!(d.estimated_pixel_width, 320.0);
        assert_eq!(d.estimated_pixel_height, 160.0);
    }

    #[test]
    fn hover_dimensions_exceeds_bounds() {
        let d = HoverDimensions::new(40, 10, 8.0, 16.0);
        assert!(!d.exceeds_bounds(400.0, 200.0));
        assert!(d.exceeds_bounds(200.0, 100.0));
    }

    #[test]
    fn size_calc_empty_text() {
        let calc = HoverContentSizeCalculator::new(8.0, 16.0);
        let dims = calc.estimate_text("");
        assert_eq!(dims.width_chars, 0);
        assert_eq!(dims.height_lines, 2); // padding
    }

    #[test]
    fn size_calc_single_line() {
        let calc = HoverContentSizeCalculator::new(8.0, 16.0);
        let dims = calc.estimate_text("hello world");
        assert_eq!(dims.width_chars, 11);
        assert_eq!(dims.height_lines, 3); // 1 line + 2 padding
    }

    #[test]
    fn size_calc_multiline() {
        let calc = HoverContentSizeCalculator::new(8.0, 16.0);
        let dims = calc.estimate_text("line 1\nline two\nline three here");
        assert_eq!(dims.height_lines, 5); // 3 lines + 2 padding
        assert_eq!(dims.width_chars, 15); // "line three here"
    }

    #[test]
    fn size_calc_wrapping() {
        let calc = HoverContentSizeCalculator::new(8.0, 16.0).with_max_width(10);
        let dims = calc.estimate_text("this is a very long line that wraps");
        assert_eq!(dims.width_chars, 10);
        assert!(dims.height_lines > 3);
    }

    #[test]
    fn size_calc_code_block() {
        let calc = HoverContentSizeCalculator::new(8.0, 16.0);
        let dims = calc.estimate_code_block("fn main() {\n    println!(\"hi\");\n}", "rust");
        assert_eq!(dims.height_lines, 7); // 3 lines + 2 padding + 2 borders
    }

    #[test]
    fn size_calc_combined() {
        let calc = HoverContentSizeCalculator::new(8.0, 16.0);
        let dims = calc.estimate_combined(&["line 1", "line 2"]);
        assert_eq!(dims.height_lines, 6); // 2 * (1 + 2 padding)
    }

    #[test]
    fn animator_fade_in() {
        let mut anim = HoverWidgetAnimator::new(200, 100);
        assert_eq!(anim.phase(), AnimationPhase::Hidden);
        anim.start_fade_in();
        assert_eq!(anim.phase(), AnimationPhase::FadingIn);
        anim.tick(100);
        assert!((anim.opacity() - 0.5).abs() < 1e-9);
        assert!(anim.is_animating());
        anim.tick(100);
        assert!((anim.opacity() - 1.0).abs() < 1e-9);
        assert_eq!(anim.phase(), AnimationPhase::Visible);
    }

    #[test]
    fn animator_fade_out() {
        let mut anim = HoverWidgetAnimator::new(100, 200);
        anim.show_immediate();
        anim.start_fade_out();
        anim.tick(100);
        assert!((anim.opacity() - 0.5).abs() < 1e-9);
        anim.tick(100);
        assert_eq!(anim.phase(), AnimationPhase::Hidden);
        assert!(!anim.is_visible());
    }

    #[test]
    fn animator_immediate_show_hide() {
        let mut anim = HoverWidgetAnimator::new(100, 100);
        anim.show_immediate();
        assert_eq!(anim.phase(), AnimationPhase::Visible);
        assert!((anim.opacity() - 1.0).abs() < 1e-9);
        anim.hide_immediate();
        assert_eq!(anim.phase(), AnimationPhase::Hidden);
    }

    #[test]
    fn animation_phase_display() {
        assert_eq!(format!("{}", AnimationPhase::Hidden), "hidden");
        assert_eq!(format!("{}", AnimationPhase::FadingIn), "fading_in");
        assert_eq!(format!("{}", AnimationPhase::Visible), "visible");
        assert_eq!(format!("{}", AnimationPhase::FadingOut), "fading_out");
    }



    #[test]
    fn hover_lru_insert_get() {
        let mut c = HoverLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn hover_lru_eviction() {
        let mut c = HoverLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn hover_lru_hit_ratio() {
        let mut c = HoverLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn hover_lru_clear() {
        let mut c = HoverLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn hover_lru_remove() {
        let mut c = HoverLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn hover_lru_peek() {
        let mut c = HoverLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn hover_fmt_list() {
        let f = HoverFmt::new(HoverFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn hover_fmt_kv() {
        let f = HoverFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn hover_fmt_section() {
        let f = HoverFmt::new(HoverFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn hover_fmt_truncate() {
        let f = HoverFmt::new(HoverFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn hover_fmt_opts_defaults() {
        let o = HoverFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    // -- hover Z-extended tests -----------------------------------------------

    #[test]
    fn z_hover_priority_weight() {
        assert_eq!(ZHoverPriority::Idle.weight(), 0);
        assert_eq!(ZHoverPriority::Normal.weight(), 2);
        assert_eq!(ZHoverPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_hover_priority_label() {
        assert_eq!(ZHoverPriority::Low.label(), "low");
        assert_eq!(ZHoverPriority::High.label(), "high");
    }

    #[test]
    fn z_hover_priority_is_elevated() {
        assert!(!ZHoverPriority::Normal.is_elevated());
        assert!(ZHoverPriority::High.is_elevated());
        assert!(ZHoverPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_hover_priority_display() {
        assert_eq!(format!("{}", ZHoverPriority::Idle), "idle");
    }

    #[test]
    fn z_hover_priority_all_asc() {
        let all = ZHoverPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZHoverPriority::Idle);
        assert_eq!(all[4], ZHoverPriority::Realtime);
    }

    #[test]
    fn z_hover_struct_new() {
        let s = ZHoverHoverDelay::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_hover_struct_toggled_clone() {
        let s = ZHoverHoverDelay::new();
        let t = s.toggled_clone();
        assert_ne!(s.active, t.active);
    }

    #[test]
    fn z_hover_rolling_hash_deterministic() {
        let h1 = z_hover_rolling_hash(b"test");
        let h2 = z_hover_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_hover_rolling_hash(b"a"), z_hover_rolling_hash(b"b"));
    }

    #[test]
    fn z_hover_pad_to_basic() {
        assert_eq!(z_hover_pad_to("hi", 5), "hi   ");
        assert_eq!(z_hover_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_hover_is_identifier_basic() {
        assert!(z_hover_is_identifier("foo_bar"));
        assert!(z_hover_is_identifier("abc123"));
        assert!(!z_hover_is_identifier(""));
        assert!(!z_hover_is_identifier("has space"));
    }

    #[test]
    fn z_hover_levenshtein_basic() {
        assert_eq!(z_hover_levenshtein("", ""), 0);
        assert_eq!(z_hover_levenshtein("abc", "abc"), 0);
        assert_eq!(z_hover_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_hover_unique_words_basic() {
        let w = z_hover_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_hover_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_hover_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_hover_common_prefix_basic() {
        assert_eq!(z_hover_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_hover_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_hover_struct_clear() {
        let mut s = ZHoverHoverDelay::new();
        s.trigger_times_ms.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_hover_rolling_hash_empty() {
        let h = z_hover_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
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


    // ---- xc_ pool / scheduler tests – block 90 ----

    #[test]
    fn xc_90_pool_new_empty() {
        let pool: super::Xc90Pool<i32> = super::Xc90Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_90_pool_release_acquire() {
        let mut pool = super::Xc90Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_90_pool_acquire_empty() {
        let mut pool: super::Xc90Pool<i32> = super::Xc90Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_90_pool_full() {
        let mut pool = super::Xc90Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_90_pool_drain() {
        let mut pool = super::Xc90Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_90_pool_stats() {
        let mut pool = super::Xc90Pool::new(8);
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
    fn xc_90_pool_clear() {
        let mut pool = super::Xc90Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_90_pool_shrink() {
        let mut pool = super::Xc90Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_90_pool_default() {
        let pool: super::Xc90Pool<String> = super::Xc90Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_90_pool_extend() {
        let mut pool = super::Xc90Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_90_pool_retain() {
        let mut pool = super::Xc90Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_90_scheduler_round_robin() {
        let mut sched = super::Xc90Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_90_scheduler_empty() {
        let mut sched = super::Xc90Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_90_scheduler_reset() {
        let mut sched = super::Xc90Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_90_scheduler_add_remove() {
        let mut sched = super::Xc90Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_90_scheduler_targets() {
        let sched = super::Xc90Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_90_hash_empty() {
        assert_eq!(super::xc_90_hash(b""), 5381);
    }

    #[test]
    fn xc_90_hash_data() {
        let h = super::xc_90_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_90_hash(b"hello"), h);
    }

    #[test]
    fn xc_90_reverse_str() {
        assert_eq!(super::xc_90_reverse("abc"), "cba");
        assert_eq!(super::xc_90_reverse(""), "");
    }


    // --- xd_4 deepening tests ---

    #[test]
    fn xd_4_sm_initial_state() {
        let sm = Xd4StateMachine::new();
        assert_eq!(sm.current_state(), Xd4State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_4_sm_valid_idle_to_running() {
        let mut sm = Xd4StateMachine::new();
        assert!(sm.transition(Xd4State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd4State::Running);
    }

    #[test]
    fn xd_4_sm_valid_running_to_paused() {
        let mut sm = Xd4StateMachine::new();
        sm.transition(Xd4State::Running).unwrap();
        assert!(sm.transition(Xd4State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd4State::Paused);
    }

    #[test]
    fn xd_4_sm_valid_running_to_done() {
        let mut sm = Xd4StateMachine::new();
        sm.transition(Xd4State::Running).unwrap();
        assert!(sm.transition(Xd4State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd4State::Done);
    }

    #[test]
    fn xd_4_sm_valid_paused_to_running() {
        let mut sm = Xd4StateMachine::new();
        sm.transition(Xd4State::Running).unwrap();
        sm.transition(Xd4State::Paused).unwrap();
        assert!(sm.transition(Xd4State::Running).is_ok());
    }

    #[test]
    fn xd_4_sm_valid_done_to_idle() {
        let mut sm = Xd4StateMachine::new();
        sm.transition(Xd4State::Running).unwrap();
        sm.transition(Xd4State::Done).unwrap();
        assert!(sm.transition(Xd4State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd4State::Idle);
    }

    #[test]
    fn xd_4_sm_invalid_idle_to_done() {
        let mut sm = Xd4StateMachine::new();
        assert!(sm.transition(Xd4State::Done).is_err());
    }

    #[test]
    fn xd_4_sm_invalid_idle_to_paused() {
        let mut sm = Xd4StateMachine::new();
        assert!(sm.transition(Xd4State::Paused).is_err());
    }

    #[test]
    fn xd_4_sm_history_tracking() {
        let mut sm = Xd4StateMachine::new();
        sm.transition(Xd4State::Running).unwrap();
        sm.transition(Xd4State::Paused).unwrap();
        sm.transition(Xd4State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd4State::Idle);
        assert_eq!(sm.history()[0].to, Xd4State::Running);
        assert_eq!(sm.history()[1].from, Xd4State::Running);
        assert_eq!(sm.history()[2].to, Xd4State::Done);
    }

    #[test]
    fn xd_4_sm_serialize_deserialize() {
        let mut sm = Xd4StateMachine::new();
        sm.transition(Xd4State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd4StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd4State::Running));
    }

    #[test]
    fn xd_4_sm_deserialize_invalid() {
        assert_eq!(Xd4StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_4_sm_reset() {
        let mut sm = Xd4StateMachine::new();
        sm.transition(Xd4State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd4State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_4_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd4EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd4Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_4_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd4EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd4Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd4Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_4_bus_unsubscribe() {
        let mut bus = Xd4EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_4_event_kind_and_payload() {
        let e = Xd4Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd4Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_4_bus_clear_history() {
        let mut bus = Xd4EventBus::new();
        bus.publish(Xd4Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_4_sm_step_counter_increments() {
        let mut sm = Xd4StateMachine::new();
        sm.transition(Xd4State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd4State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}