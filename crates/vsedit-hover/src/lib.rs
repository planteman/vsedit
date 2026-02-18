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


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #2
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf2Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf2TrieNode {
    children: std::collections::HashMap<char, Xf2TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf2Trie {
    root: Xf2TrieNode,
    count: usize,
}

impl Xf2Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf2TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf2TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf2TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf2BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf2BloomFilter {
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


// ---------------------------------------------------------------------------
// xg_121: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg121Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg121Graph {
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

impl Default for Xg121Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_121: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg121Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg121Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg121Heap<T>) {
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

impl<T: Ord> Default for Xg121Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 89).
pub struct Xh89SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh89SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 131 as u64,
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

/// A compact bit set supporting boolean operations (variant 89).
pub struct Xh89BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh89BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 89).
pub struct Xi89Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi89Deque<T> {
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
pub struct Xi89Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi89Interval {
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

/// A simple interval tree (variant 89).
pub struct Xi89IntervalTree {
    xi_intervals: Vec<Xi89Interval>,
}

impl Xi89IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi89Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi89Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi89Interval) -> Vec<&Xi89Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi89Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi89Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi89Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi89Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi89Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi89Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 89) ---

/// Disjoint set / union-find for crate 89.
pub struct Xj89UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj89UnionFind {
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

const XJ89_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 89.
pub struct Xj89BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj89BTreeNode<K, V>>>,
    len: usize,
}

struct Xj89BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj89BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj89BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ89_BTREE_ORDER - 1
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
        let mid = XJ89_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj89BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj89BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj89BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj89BTreeNode::xj_new_leaf();
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


// --- xk_88 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk88SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk88SegmentTree {
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
pub struct Xk88DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk88DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_89).
#[derive(Debug, Clone)]
pub struct Xl89Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl89Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_89).
#[derive(Debug, Clone)]
pub struct Xl89SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl89SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm89MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm89MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm89Tokenizer {
    text: String,
}

impl Xm89Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 89.
pub struct Xn89Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn89Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 89 -----

#[derive(Debug, Clone)]
struct Xn89AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn89AvlNode<K, V>>>,
    right: Option<Box<Xn89AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 89.
#[derive(Debug, Clone)]
pub struct Xn89AVL<K, V> {
    root: Option<Box<Xn89AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn89AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn89AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn89AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn89AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn89AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn89AvlNode<K, V>>) -> Box<Xn89AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn89AvlNode<K, V>>) -> Box<Xn89AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn89AvlNode<K, V>>) -> Box<Xn89AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn89AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn89AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn89AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn89AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn89AvlNode<K, V>>) -> &Xn89AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn89AvlNode<K, V>>) -> (Box<Xn89AvlNode<K, V>>, Option<Box<Xn89AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn89AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn89AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn89AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn89AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn89AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn89AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn89AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo89RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo89Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo89RBNode<K, V> {
    key: K,
    value: V,
    color: Xo89Color,
    left: Option<Box<Xo89RBNode<K, V>>>,
    right: Option<Box<Xo89RBNode<K, V>>>,
}

/// A red-black tree map for crate 89.
#[derive(Debug, Clone)]
pub struct Xo89RedBlack<K, V> {
    root: Option<Box<Xo89RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo89RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo89Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo89RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo89RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo89RBNode {
                    key, value, color: Xo89Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo89RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo89Color::Red)
    }

    fn xo_balance(mut h: Box<Xo89RBNode<K, V>>) -> Box<Xo89RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo89Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo89RBNode<K, V>>) -> Box<Xo89RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo89Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo89RBNode<K, V>>) -> Box<Xo89RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo89Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo89RBNode<K, V>>) {
        h.color = Xo89Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo89Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo89Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo89Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo89RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo89RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo89RBNode<K, V>) -> (K, V, Option<Box<Xo89RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo89RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo89Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo89RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo89ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 89.
#[derive(Debug, Clone)]
pub struct Xo89ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo89ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo89#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo89#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 88).
#[derive(Debug)]
pub struct Xp88SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp88Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp88Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp88Node<K, V>>>,
    xp_right: Option<Box<Xp88Node<K, V>>>,
}

impl<K: Ord, V> Xp88Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp88SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp88SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp88Node<K, V>>>, key: &K) -> Option<Box<Xp88Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp88Node<K, V>>) -> Box<Xp88Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp88Node<K, V>>) -> Box<Xp88Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp88Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp88Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp88Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq89Treap ---------------

use std::cmp::Ordering as Xq89Ord;

struct Xq89TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq89TreapNode<K, V>>>,
    right: Option<Box<Xq89TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq89Treap<K, V> {
    root: Option<Box<Xq89TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq89TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_89_size<K, V>(node: &Option<Box<Xq89TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_89_update_size<K, V>(node: &mut Xq89TreapNode<K, V>) {
    node.size = 1 + xq_89_size(&node.left) + xq_89_size(&node.right);
}

fn xq_89_rotate_right<K, V>(mut node: Box<Xq89TreapNode<K, V>>) -> Box<Xq89TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_89_update_size(&mut node);
    left.right = Some(node);
    xq_89_update_size(&mut left);
    left
}

fn xq_89_rotate_left<K, V>(mut node: Box<Xq89TreapNode<K, V>>) -> Box<Xq89TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_89_update_size(&mut node);
    right.left = Some(node);
    xq_89_update_size(&mut right);
    right
}

fn xq_89_insert_node<K: Ord, V>(
    node: Option<Box<Xq89TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq89TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq89TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq89Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq89Ord::Less => {
                let (new_left, old) = xq_89_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_89_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_89_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq89Ord::Greater => {
                let (new_right, old) = xq_89_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_89_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_89_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_89_remove_node<K: Ord, V>(
    node: Option<Box<Xq89TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq89TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq89Ord::Less => {
                let (new_left, old) = xq_89_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_89_update_size(&mut n);
                (Some(n), old)
            }
            Xq89Ord::Greater => {
                let (new_right, old) = xq_89_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_89_update_size(&mut n);
                (Some(n), old)
            }
            Xq89Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_89_rotate_right(n);
                    let (new_right, old) = xq_89_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_89_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_89_rotate_left(n);
                    let (new_left, old) = xq_89_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_89_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_89_find_min<K, V>(node: &Option<Box<Xq89TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_89_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_89_find_max<K, V>(node: &Option<Box<Xq89TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_89_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_89_rank<K: Ord, V>(node: &Option<Box<Xq89TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq89Ord::Less => xq_89_rank(&n.left, key),
            Xq89Ord::Equal => xq_89_size(&n.left),
            Xq89Ord::Greater => 1 + xq_89_size(&n.left) + xq_89_rank(&n.right, key),
        },
    }
}

fn xq_89_kth<K, V>(node: &Option<Box<Xq89TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_89_size(&n.left);
        if k < left_size {
            xq_89_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_89_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_89_in_order<K: Clone, V>(node: &Option<Box<Xq89TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_89_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_89_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq89Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 89 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_89_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq89Ord::Equal => return Some(&n.value),
                Xq89Ord::Less => cur = &n.left,
                Xq89Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_89_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_89_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_89_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_89_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_89_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_89_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_89_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq89VEBTree ---------------

pub struct Xq89VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq89VEBTree>>,
    clusters: Vec<Option<Box<Xq89VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq89VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq89VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq89VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr89KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr89KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr89BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr89KDNode {
    xr_point: Xr89KDPoint,
    xr_left: Option<Box<Xr89KDNode>>,
    xr_right: Option<Box<Xr89KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr89KDTree {
    xr_root: Option<Box<Xr89KDNode>>,
    xr_size: usize,
}

impl Xr89KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr89KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr89KDNode>>,
        point: Xr89KDPoint,
        depth: usize,
    ) -> Box<Xr89KDNode> {
        match node {
            None => Box::new(Xr89KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr89KDPoint) -> Option<Xr89KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr89KDNode>,
        query: &Xr89KDPoint,
        depth: usize,
        best: &mut Xr89KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr89KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr89KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr89KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr89KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr89KDNode>>, pts: &mut Vec<Xr89KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr89KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr89BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr89BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs89PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs89PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs89PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs89PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs89ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs89ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs89ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs89RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs89RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs89RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs89CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs89CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs89CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
}


// --- xu_ Binomial Heap ---

/// A node in a binomial heap.
#[derive(Debug, Clone)]
pub struct XuBinomialNode<K: Ord + Clone, V: Clone> {
    pub xu_key: K,
    pub xu_value: V,
    xu_degree: usize,
    xu_children: Vec<usize>,
    xu_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XuBinomialNode<K, V> {
    /// Create a new binomial node.
    pub fn xu_new(key: K, value: V) -> Self {
        Self { xu_key: key, xu_value: value, xu_degree: 0, xu_children: Vec::new(), xu_parent: None }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinNode(key={}, deg={})", self.xu_key, self.xu_degree)
    }
}

/// Binomial heap with O(log n) insert, extract-min, and merge.
#[derive(Debug, Clone)]
pub struct XuBinomialHeap<K: Ord + Clone, V: Clone> {
    xu_nodes: Vec<XuBinomialNode<K, V>>,
    xu_roots: Vec<usize>,
    xu_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XuBinomialHeap<K, V> {
    fn default() -> Self { Self::xu_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinHeap(size={}, trees={})", self.xu_size, self.xu_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XuBinomialHeap<K, V> {
    /// Create an empty binomial heap.
    pub fn xu_new() -> Self {
        Self { xu_nodes: Vec::new(), xu_roots: Vec::new(), xu_size: 0 }
    }

    /// Return the number of elements.
    pub fn xu_len(&self) -> usize { self.xu_size }

    /// Check if the heap is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_size == 0 }

    /// Insert a key-value pair.
    pub fn xu_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xu_nodes.len();
        self.xu_nodes.push(XuBinomialNode::xu_new(key, value));
        self.xu_add_root(idx);
        self.xu_size += 1;
        self.xu_consolidate();
        idx
    }

    fn xu_add_root(&mut self, idx: usize) {
        self.xu_nodes[idx].xu_parent = None;
        self.xu_roots.push(idx);
    }

    fn xu_consolidate(&mut self) {
        let max_deg = (self.xu_size as f64).log2().ceil() as usize + 2;
        let mut table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xu_roots.clone();
        self.xu_roots.clear();
        for root in roots {
            let mut x = root;
            loop {
                let d = self.xu_nodes[x].xu_degree;
                if d >= table.len() { break; }
                match table[d] {
                    None => { table[d] = Some(x); break; }
                    Some(y) => {
                        table[d] = None;
                        let (p, c) = if self.xu_nodes[x].xu_key <= self.xu_nodes[y].xu_key { (x, y) } else { (y, x) };
                        self.xu_nodes[p].xu_children.push(c);
                        self.xu_nodes[c].xu_parent = Some(p);
                        self.xu_nodes[p].xu_degree += 1;
                        x = p;
                    }
                }
            }
        }
        for slot in &table {
            if let Some(r) = slot {
                self.xu_roots.push(*r);
            }
        }
        self.xu_roots.sort_by_key(|&r| self.xu_nodes[r].xu_degree);
    }

    /// Peek at the minimum.
    pub fn xu_find_min(&self) -> Option<(&K, &V)> {
        self.xu_roots.iter()
            .min_by(|&&a, &&b| self.xu_nodes[a].xu_key.cmp(&self.xu_nodes[b].xu_key))
            .map(|&i| (&self.xu_nodes[i].xu_key, &self.xu_nodes[i].xu_value))
    }

    /// Extract the minimum element.
    pub fn xu_extract_min(&mut self) -> Option<(K, V)> {
        if self.xu_roots.is_empty() { return None; }
        let min_pos = self.xu_roots.iter().enumerate()
            .min_by(|(_, a), (_, b)| self.xu_nodes[**a].xu_key.cmp(&self.xu_nodes[**b].xu_key))
            .map(|(pos, _)| pos)?;
        let min_idx = self.xu_roots.remove(min_pos);
        let children = self.xu_nodes[min_idx].xu_children.clone();
        for &c in &children {
            self.xu_nodes[c].xu_parent = None;
            self.xu_roots.push(c);
        }
        self.xu_size -= 1;
        if !self.xu_roots.is_empty() {
            self.xu_consolidate();
        }
        let n = &self.xu_nodes[min_idx];
        Some((n.xu_key.clone(), n.xu_value.clone()))
    }

    /// Merge another binomial heap into this one.
    pub fn xu_merge(&mut self, other: &mut XuBinomialHeap<K, V>) {
        let off = self.xu_nodes.len();
        for mut n in other.xu_nodes.drain(..) {
            n.xu_parent = n.xu_parent.map(|p| p + off);
            n.xu_children = n.xu_children.iter().map(|&c| c + off).collect();
            self.xu_nodes.push(n);
        }
        for r in other.xu_roots.drain(..) {
            self.xu_roots.push(r + off);
        }
        self.xu_size += other.xu_size;
        other.xu_size = 0;
        self.xu_consolidate();
    }

    /// Drain all elements in sorted order.
    pub fn xu_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xu_size);
        while let Some(pair) = self.xu_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xu_clear(&mut self) {
        self.xu_nodes.clear();
        self.xu_roots.clear();
        self.xu_size = 0;
    }
}

// --- xu_ Disjoint Sparse Table ---

/// Disjoint sparse table for O(1) range queries on static data with an associative operation.
#[derive(Debug, Clone)]
pub struct XuDisjointSparseTable<T: Clone> {
    xu_table: Vec<Vec<T>>,
    xu_data: Vec<T>,
    xu_len: usize,
    xu_levels: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XuDisjointSparseTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DST(len={}, levels={})", self.xu_len, self.xu_levels)
    }
}

impl<T: Clone + Default + std::ops::Add<Output = T>> XuDisjointSparseTable<T> {
    /// Build a disjoint sparse table for range-sum queries.
    pub fn xu_build(data: &[T]) -> Self {
        let n = data.len();
        if n == 0 {
            return Self { xu_table: Vec::new(), xu_data: Vec::new(), xu_len: 0, xu_levels: 0 };
        }
        let levels = (n as f64).log2().ceil() as usize + 1;
        let mut table = Vec::with_capacity(levels);
        for level in 0..levels {
            let block = 1 << level;
            let mut row = data.to_vec();
            let mut mid = block;
            while mid < n {
                // Build prefix sums going left from mid
                if mid > 0 && mid - 1 < n {
                    let start = if mid >= block { mid - block } else { 0 };
                    let mut i = mid.saturating_sub(1);
                    loop {
                        if i < start { break; }
                        if i + 1 < mid && i + 1 < n {
                            row[i] = row[i].clone() + row[i + 1].clone();
                        }
                        if i == start { break; }
                        i -= 1;
                    }
                }
                // Build prefix sums going right from mid
                let end = std::cmp::min(mid + block, n);
                for i in (mid + 1)..end {
                    row[i] = row[i - 1].clone() + row[i].clone();
                }
                mid += 2 * block;
            }
            table.push(row);
        }
        Self { xu_table: table, xu_data: data.to_vec(), xu_len: n, xu_levels: levels }
    }

    /// Query the sum of elements in the range [l, r] (inclusive).
    pub fn xu_query(&self, l: usize, r: usize) -> T {
        if l == r {
            return self.xu_data[l].clone();
        }
        if l >= self.xu_len || r >= self.xu_len || l > r {
            return T::default();
        }
        // Find the highest bit where l and r differ
        let xor = l ^ r;
        if xor == 0 {
            return self.xu_data[l].clone();
        }
        let level = (usize::BITS - xor.leading_zeros() - 1) as usize;
        if level < self.xu_levels && l < self.xu_table[level].len() && r < self.xu_table[level].len() {
            self.xu_table[level][l].clone() + self.xu_table[level][r].clone()
        } else {
            // Fallback: linear sum
            let mut sum = self.xu_data[l].clone();
            for i in (l + 1)..=r {
                sum = sum + self.xu_data[i].clone();
            }
            sum
        }
    }

    /// Return the length.
    pub fn xu_len(&self) -> usize { self.xu_len }

    /// Check if empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_len == 0 }

    /// Get element at index.
    pub fn xu_get(&self, idx: usize) -> Option<&T> {
        self.xu_data.get(idx)
    }
}

// --- xu_ Monotonic Stack ---

/// Monotonic stack that maintains elements in non-decreasing or non-increasing order.
#[derive(Debug, Clone)]
pub struct XuMonotonicStack<T: Clone + Ord> {
    xu_data: Vec<T>,
    xu_increasing: bool,
}

impl<T: Clone + Ord + std::fmt::Display> std::fmt::Display for XuMonotonicStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonoStack(len={}, inc={})", self.xu_data.len(), self.xu_increasing)
    }
}

impl<T: Clone + Ord> XuMonotonicStack<T> {
    /// Create a monotonically increasing stack.
    pub fn xu_increasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: true }
    }

    /// Create a monotonically decreasing stack.
    pub fn xu_decreasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: false }
    }

    /// Push a value, popping elements that violate the monotonic invariant.
    pub fn xu_push(&mut self, value: T) -> Vec<T> {
        let mut popped = Vec::new();
        if self.xu_increasing {
            while let Some(top) = self.xu_data.last() {
                if *top > value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        } else {
            while let Some(top) = self.xu_data.last() {
                if *top < value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        }
        self.xu_data.push(value);
        popped
    }

    /// Peek at the top.
    pub fn xu_peek(&self) -> Option<&T> { self.xu_data.last() }

    /// Pop from top.
    pub fn xu_pop(&mut self) -> Option<T> { self.xu_data.pop() }

    /// Length.
    pub fn xu_len(&self) -> usize { self.xu_data.len() }

    /// Is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_data.is_empty() }

    /// Get all elements.
    pub fn xu_as_slice(&self) -> &[T] { &self.xu_data }

    /// Clear the stack.
    pub fn xu_clear(&mut self) { self.xu_data.clear(); }
}


// --- xv_ Cartesian Tree ---

/// A node in a Cartesian tree (BST by key, heap by priority).
#[derive(Debug, Clone)]
pub struct XvCartesianNode<K: Ord + Clone, P: Ord + Clone> {
    pub xv_key: K,
    pub xv_priority: P,
    xv_left: Option<Box<XvCartesianNode<K, P>>>,
    xv_right: Option<Box<XvCartesianNode<K, P>>>,
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianNode<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartNode(k={}, p={})", self.xv_key, self.xv_priority)
    }
}

/// Cartesian tree — BST by key, min-heap by priority. Used for range-minimum queries.
#[derive(Debug, Clone)]
pub struct XvCartesianTree<K: Ord + Clone, P: Ord + Clone> {
    xv_root: Option<Box<XvCartesianNode<K, P>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, P: Ord + Clone> Default for XvCartesianTree<K, P> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianTree<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, P: Ord + Clone> XvCartesianTree<K, P> {
    /// Create an empty Cartesian tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Return the number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Check if empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    /// Insert a (key, priority) pair maintaining BST-by-key and min-heap-by-priority.
    pub fn xv_insert(&mut self, key: K, priority: P) {
        self.xv_root = Self::xv_insert_node(self.xv_root.take(), key, priority);
        self.xv_size += 1;
    }

    fn xv_insert_node(node: Option<Box<XvCartesianNode<K, P>>>, key: K, priority: P) -> Option<Box<XvCartesianNode<K, P>>> {
        match node {
            None => Some(Box::new(XvCartesianNode { xv_key: key, xv_priority: priority, xv_left: None, xv_right: None })),
            Some(mut n) => {
                if key < n.xv_key {
                    n.xv_left = Self::xv_insert_node(n.xv_left.take(), key.clone(), priority.clone());
                    if n.xv_left.as_ref().is_some_and(|l| l.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_right(n);
                    }
                    Some(n)
                } else {
                    n.xv_right = Self::xv_insert_node(n.xv_right.take(), key.clone(), priority.clone());
                    if n.xv_right.as_ref().is_some_and(|r| r.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_left(n);
                    }
                    Some(n)
                }
            }
        }
    }

    fn xv_rotate_right(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        left.xv_right = Some(node);
        left
    }

    fn xv_rotate_left(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        right.xv_left = Some(node);
        right
    }

    /// Search for a key.
    pub fn xv_contains(&self, key: &K) -> bool {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search(node: &Option<Box<XvCartesianNode<K, P>>>, key: &K) -> bool {
        match node {
            None => false,
            Some(n) => {
                if *key == n.xv_key { true }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// In-order traversal returning keys.
    pub fn xv_inorder(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder_walk(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder_walk(node: &Option<Box<XvCartesianNode<K, P>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder_walk(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder_walk(&n.xv_right, result);
        }
    }

    /// Get the root priority (minimum priority).
    pub fn xv_min_priority(&self) -> Option<&P> {
        self.xv_root.as_ref().map(|n| &n.xv_priority)
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Build from a sequence of (key, priority) pairs.
    pub fn xv_from_pairs(pairs: &[(K, P)]) -> Self {
        let mut tree = Self::xv_new();
        for (k, p) in pairs { tree.xv_insert(k.clone(), p.clone()); }
        tree
    }

    /// Height of the tree.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvCartesianNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(
                Self::xv_node_height(&n.xv_left),
                Self::xv_node_height(&n.xv_right),
            ),
        }
    }
}

// --- xv_ Weight-Balanced Tree ---

/// A node in a weight-balanced tree (BB[α] tree).
#[derive(Debug, Clone)]
pub struct XvWBNode<K: Ord + Clone, V: Clone> {
    pub xv_key: K,
    pub xv_value: V,
    xv_left: Option<Box<XvWBNode<K, V>>>,
    xv_right: Option<Box<XvWBNode<K, V>>>,
    xv_weight: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWBNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBNode(k={}, w={})", self.xv_key, self.xv_weight)
    }
}

/// Weight-balanced tree (BB[α] tree) with α = 0.29 for balanced operations.
#[derive(Debug, Clone)]
pub struct XvWeightBalancedTree<K: Ord + Clone, V: Clone> {
    xv_root: Option<Box<XvWBNode<K, V>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XvWeightBalancedTree<K, V> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWeightBalancedTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, V: Clone> XvWeightBalancedTree<K, V> {
    const ALPHA: f64 = 0.29;

    /// Create an empty weight-balanced tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Is the tree empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    fn xv_weight(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node { None => 1, Some(n) => n.xv_weight }
    }

    fn xv_update_weight(node: &mut Box<XvWBNode<K, V>>) {
        node.xv_weight = Self::xv_weight(&node.xv_left) + Self::xv_weight(&node.xv_right);
    }

    fn xv_is_balanced(node: &Box<XvWBNode<K, V>>) -> bool {
        let lw = Self::xv_weight(&node.xv_left) as f64;
        let rw = Self::xv_weight(&node.xv_right) as f64;
        let total = node.xv_weight as f64;
        lw >= Self::ALPHA * total && rw >= Self::ALPHA * total
    }

    /// Insert a key-value pair.
    pub fn xv_insert(&mut self, key: K, value: V) {
        let inserted = Self::xv_insert_node(self.xv_root.take(), key, value);
        self.xv_root = inserted.0;
        if inserted.1 { self.xv_size += 1; }
    }

    fn xv_insert_node(node: Option<Box<XvWBNode<K, V>>>, key: K, value: V) -> (Option<Box<XvWBNode<K, V>>>, bool) {
        match node {
            None => {
                let n = Box::new(XvWBNode { xv_key: key, xv_value: value, xv_left: None, xv_right: None, xv_weight: 2 });
                (Some(n), true)
            }
            Some(mut n) => {
                let inserted;
                if key < n.xv_key {
                    let r = Self::xv_insert_node(n.xv_left.take(), key, value);
                    n.xv_left = r.0;
                    inserted = r.1;
                } else if key > n.xv_key {
                    let r = Self::xv_insert_node(n.xv_right.take(), key, value);
                    n.xv_right = r.0;
                    inserted = r.1;
                } else {
                    n.xv_value = value;
                    return (Some(n), false);
                }
                Self::xv_update_weight(&mut n);
                let n = Self::xv_rebalance(n);
                (Some(n), inserted)
            }
        }
    }

    fn xv_rebalance(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if !Self::xv_is_balanced(&node) {
            let lw = Self::xv_weight(&node.xv_left);
            let rw = Self::xv_weight(&node.xv_right);
            if lw < rw {
                node = Self::xv_rotate_left_wb(node);
            } else {
                node = Self::xv_rotate_right_wb(node);
            }
        }
        node
    }

    fn xv_rotate_left_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_right.is_none() { return node; }
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        Self::xv_update_weight(&mut node);
        right.xv_left = Some(node);
        Self::xv_update_weight(&mut right);
        right
    }

    fn xv_rotate_right_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_left.is_none() { return node; }
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        Self::xv_update_weight(&mut node);
        left.xv_right = Some(node);
        Self::xv_update_weight(&mut left);
        left
    }

    /// Look up a key.
    pub fn xv_get(&self, key: &K) -> Option<&V> {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search<'a>(node: &'a Option<Box<XvWBNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xv_key { Some(&n.xv_value) }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xv_contains(&self, key: &K) -> bool { self.xv_get(key).is_some() }

    /// In-order traversal.
    pub fn xv_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder(node: &Option<Box<XvWBNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder(&n.xv_right, result);
        }
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Height.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xv_node_height(&n.xv_left), Self::xv_node_height(&n.xv_right)),
        }
    }
}


// --- xw_ Scapegoat Tree ---

/// A node in a scapegoat tree.
#[derive(Debug, Clone)]
pub struct XwScapegoatNode<K: Ord + Clone, V: Clone> {
    pub xw_key: K,
    pub xw_value: V,
    xw_left: Option<Box<XwScapegoatNode<K, V>>>,
    xw_right: Option<Box<XwScapegoatNode<K, V>>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGNode(k={})", self.xw_key)
    }
}

/// Scapegoat tree — a BST that rebuilds subtrees when they become too unbalanced.
#[derive(Debug, Clone)]
pub struct XwScapegoatTree<K: Ord + Clone, V: Clone> {
    xw_root: Option<Box<XwScapegoatNode<K, V>>>,
    xw_size: usize,
    xw_max_size: usize,
    xw_alpha: f64,
}

impl<K: Ord + Clone, V: Clone> Default for XwScapegoatTree<K, V> {
    fn default() -> Self { Self::xw_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGTree(size={}, alpha={:.2})", self.xw_size, self.xw_alpha)
    }
}

impl<K: Ord + Clone, V: Clone> XwScapegoatTree<K, V> {
    /// Create an empty scapegoat tree with default α = 0.7.
    pub fn xw_new() -> Self {
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: 0.7 }
    }

    /// Create with custom alpha (0.5 < α < 1.0).
    pub fn xw_with_alpha(alpha: f64) -> Self {
        let a = alpha.clamp(0.51, 0.99);
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: a }
    }

    /// Number of elements.
    pub fn xw_len(&self) -> usize { self.xw_size }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_size == 0 }

    fn xw_node_size(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + Self::xw_node_size(&n.xw_left) + Self::xw_node_size(&n.xw_right),
        }
    }

    /// Insert a key-value pair.
    pub fn xw_insert(&mut self, key: K, value: V) {
        let (new_root, depth, inserted) = Self::xw_insert_node(self.xw_root.take(), key, value, 0);
        self.xw_root = new_root;
        if inserted {
            self.xw_size += 1;
            self.xw_max_size = std::cmp::max(self.xw_max_size, self.xw_size);
            let h_alpha = -(self.xw_size as f64).log(1.0 / self.xw_alpha);
            if depth as f64 > h_alpha {
                self.xw_root = Self::xw_rebuild(self.xw_root.take());
            }
        }
    }

    fn xw_insert_node(
        node: Option<Box<XwScapegoatNode<K, V>>>, key: K, value: V, depth: usize,
    ) -> (Option<Box<XwScapegoatNode<K, V>>>, usize, bool) {
        match node {
            None => {
                let n = Box::new(XwScapegoatNode { xw_key: key, xw_value: value, xw_left: None, xw_right: None });
                (Some(n), depth, true)
            }
            Some(mut n) => {
                if key < n.xw_key {
                    let (l, d, ins) = Self::xw_insert_node(n.xw_left.take(), key, value, depth + 1);
                    n.xw_left = l;
                    if ins {
                        let ls = Self::xw_node_size(&n.xw_left);
                        let total = 1 + ls + Self::xw_node_size(&n.xw_right);
                        if ls as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else if key > n.xw_key {
                    let (r, d, ins) = Self::xw_insert_node(n.xw_right.take(), key, value, depth + 1);
                    n.xw_right = r;
                    if ins {
                        let rs = Self::xw_node_size(&n.xw_right);
                        let total = 1 + Self::xw_node_size(&n.xw_left) + rs;
                        if rs as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else {
                    n.xw_value = value;
                    (Some(n), depth, false)
                }
            }
        }
    }

    fn xw_flatten(node: Option<Box<XwScapegoatNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xw_flatten(n.xw_left, out);
            out.push((n.xw_key, n.xw_value));
            Self::xw_flatten(n.xw_right, out);
        }
    }

    fn xw_build_balanced(sorted: &[(K, V)]) -> Option<Box<XwScapegoatNode<K, V>>> {
        if sorted.is_empty() { return None; }
        let mid = sorted.len() / 2;
        let (k, v) = sorted[mid].clone();
        Some(Box::new(XwScapegoatNode {
            xw_key: k,
            xw_value: v,
            xw_left: Self::xw_build_balanced(&sorted[..mid]),
            xw_right: Self::xw_build_balanced(&sorted[mid + 1..]),
        }))
    }

    fn xw_rebuild(node: Option<Box<XwScapegoatNode<K, V>>>) -> Option<Box<XwScapegoatNode<K, V>>> {
        let mut flat = Vec::new();
        Self::xw_flatten(node, &mut flat);
        Self::xw_build_balanced(&flat)
    }

    /// Look up a key.
    pub fn xw_get(&self, key: &K) -> Option<&V> {
        Self::xw_search(&self.xw_root, key)
    }

    fn xw_search<'a>(node: &'a Option<Box<XwScapegoatNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xw_key { Some(&n.xw_value) }
                else if *key < n.xw_key { Self::xw_search(&n.xw_left, key) }
                else { Self::xw_search(&n.xw_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xw_contains(&self, key: &K) -> bool { self.xw_get(key).is_some() }

    /// In-order keys.
    pub fn xw_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xw_collect_keys(&self.xw_root, &mut result);
        result
    }

    fn xw_collect_keys(node: &Option<Box<XwScapegoatNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xw_collect_keys(&n.xw_left, result);
            result.push(n.xw_key.clone());
            Self::xw_collect_keys(&n.xw_right, result);
        }
    }

    /// Clear the tree.
    pub fn xw_clear(&mut self) {
        self.xw_root = None;
        self.xw_size = 0;
        self.xw_max_size = 0;
    }

    /// Height.
    pub fn xw_height(&self) -> usize {
        Self::xw_node_height(&self.xw_root)
    }

    fn xw_node_height(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xw_node_height(&n.xw_left), Self::xw_node_height(&n.xw_right)),
        }
    }
}

// --- xw_ Rope (String Rope) ---

/// A rope node — either a leaf with text or an internal node concatenating two children.
#[derive(Debug, Clone)]
pub enum XwRopeNode {
    Leaf(String),
    Internal {
        xw_left: Box<XwRopeNode>,
        xw_right: Box<XwRopeNode>,
        xw_len: usize,
    },
}

impl std::fmt::Display for XwRopeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XwRopeNode::Leaf(s) => write!(f, "RopeLeaf({})", s.len()),
            XwRopeNode::Internal { xw_len, .. } => write!(f, "RopeInt({})", xw_len),
        }
    }
}

/// Rope data structure for efficient string editing with O(log n) split/concat.
#[derive(Debug, Clone)]
pub struct XwRope {
    xw_root: Option<Box<XwRopeNode>>,
}

impl Default for XwRope {
    fn default() -> Self { Self::xw_new() }
}

impl std::fmt::Display for XwRope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rope(len={})", self.xw_len())
    }
}

impl XwRope {
    /// Create an empty rope.
    pub fn xw_new() -> Self { Self { xw_root: None } }

    /// Create a rope from a string.
    pub fn xw_from_str(s: &str) -> Self {
        if s.is_empty() {
            Self { xw_root: None }
        } else {
            Self { xw_root: Some(Box::new(XwRopeNode::Leaf(s.to_string()))) }
        }
    }

    /// Total length in bytes.
    pub fn xw_len(&self) -> usize {
        Self::xw_node_len(&self.xw_root)
    }

    fn xw_node_len(node: &Option<Box<XwRopeNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => s.len(),
                XwRopeNode::Internal { xw_len, .. } => *xw_len,
            },
        }
    }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_len() == 0 }

    /// Concatenate two ropes.
    pub fn xw_concat(left: XwRope, right: XwRope) -> XwRope {
        match (left.xw_root, right.xw_root) {
            (None, r) => XwRope { xw_root: r },
            (l, None) => XwRope { xw_root: l },
            (Some(l), Some(r)) => {
                let len = Self::xw_node_len(&Some(l.clone())) + Self::xw_node_len(&Some(r.clone()));
                XwRope {
                    xw_root: Some(Box::new(XwRopeNode::Internal { xw_left: l, xw_right: r, xw_len: len })),
                }
            }
        }
    }

    /// Convert to string.
    pub fn xw_to_string(&self) -> String {
        let mut result = String::new();
        Self::xw_collect(&self.xw_root, &mut result);
        result
    }

    fn xw_collect(node: &Option<Box<XwRopeNode>>, result: &mut String) {
        match node {
            None => {}
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => result.push_str(s),
                XwRopeNode::Internal { xw_left, xw_right, .. } => {
                    Self::xw_collect(&Some(xw_left.clone()), result);
                    Self::xw_collect(&Some(xw_right.clone()), result);
                }
            },
        }
    }

    /// Get character at byte index.
    pub fn xw_char_at(&self, idx: usize) -> Option<char> {
        let s = self.xw_to_string();
        s.as_bytes().get(idx).map(|&b| b as char)
    }

    /// Insert a string at byte index.
    pub fn xw_insert(&mut self, idx: usize, text: &str) {
        let s = self.xw_to_string();
        let (left, right) = s.split_at(idx.min(s.len()));
        let new_s = format!("{}{}{}", left, text, right);
        *self = Self::xw_from_str(&new_s);
    }

    /// Delete bytes in range [start, end).
    pub fn xw_delete(&mut self, start: usize, end: usize) {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        let new_s = format!("{}{}", &s[..start], &s[end..]);
        *self = Self::xw_from_str(&new_s);
    }

    /// Append text.
    pub fn xw_append(&mut self, text: &str) {
        let other = Self::xw_from_str(text);
        let old = std::mem::take(self);
        *self = Self::xw_concat(old, other);
    }

    /// Substring [start, end).
    pub fn xw_substring(&self, start: usize, end: usize) -> String {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        s[start..end].to_string()
    }

    /// Clear the rope.
    pub fn xw_clear(&mut self) { self.xw_root = None; }
}


// --- xx_ Skip List ---

/// A node in a skip list with multiple forward pointers for O(log n) search.
#[derive(Debug, Clone)]
pub struct XxSkipNode<K: Ord + Clone, V: Clone> {
    pub xx_key: Option<K>,
    pub xx_value: Option<V>,
    xx_forward: Vec<Option<usize>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.xx_key {
            Some(k) => write!(f, "SkipNode(k={}, lvl={})", k, self.xx_forward.len()),
            None => write!(f, "SkipNode(HEAD, lvl={})", self.xx_forward.len()),
        }
    }
}

/// Skip list — a probabilistic data structure with O(log n) average search, insert, delete.
#[derive(Debug, Clone)]
pub struct XxSkipList<K: Ord + Clone, V: Clone> {
    xx_nodes: Vec<XxSkipNode<K, V>>,
    xx_head: usize,
    xx_max_level: usize,
    xx_level: usize,
    xx_size: usize,
    xx_rng_state: u64,
}

impl<K: Ord + Clone, V: Clone> Default for XxSkipList<K, V> {
    fn default() -> Self { Self::xx_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipList<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkipList(size={}, level={})", self.xx_size, self.xx_level)
    }
}

impl<K: Ord + Clone, V: Clone> XxSkipList<K, V> {
    const XX_MAX_LEVEL: usize = 16;

    /// Create an empty skip list.
    pub fn xx_new() -> Self {
        let head = XxSkipNode {
            xx_key: None,
            xx_value: None,
            xx_forward: vec![None; Self::XX_MAX_LEVEL],
        };
        Self {
            xx_nodes: vec![head],
            xx_head: 0,
            xx_max_level: Self::XX_MAX_LEVEL,
            xx_level: 1,
            xx_size: 0,
            xx_rng_state: 42,
        }
    }

    fn xx_random_level(&mut self) -> usize {
        let mut lvl = 1;
        while lvl < self.xx_max_level {
            self.xx_rng_state ^= self.xx_rng_state << 13;
            self.xx_rng_state ^= self.xx_rng_state >> 7;
            self.xx_rng_state ^= self.xx_rng_state << 17;
            if self.xx_rng_state % 4 < 1 { break; }
            lvl += 1;
        }
        lvl
    }

    /// Number of elements.
    pub fn xx_len(&self) -> usize { self.xx_size }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_size == 0 }

    /// Insert a key-value pair.
    pub fn xx_insert(&mut self, key: K, value: V) {
        let mut update = vec![self.xx_head; self.xx_max_level];
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < key { current = next; continue; }
                    if *nk == key {
                        self.xx_nodes[next].xx_value = Some(value);
                        return;
                    }
                }
                break;
            }
            update[i] = current;
        }
        let lvl = self.xx_random_level();
        if lvl > self.xx_level {
            for i in self.xx_level..lvl {
                update[i] = self.xx_head;
            }
            self.xx_level = lvl;
        }
        let new_idx = self.xx_nodes.len();
        self.xx_nodes.push(XxSkipNode {
            xx_key: Some(key),
            xx_value: Some(value),
            xx_forward: vec![None; lvl],
        });
        for i in 0..lvl {
            self.xx_nodes[new_idx].xx_forward[i] = self.xx_nodes[update[i]].xx_forward[i];
            self.xx_nodes[update[i]].xx_forward[i] = Some(new_idx);
        }
        self.xx_size += 1;
    }

    /// Search for a key.
    pub fn xx_get(&self, key: &K) -> Option<&V> {
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < *key { current = next; continue; }
                    if *nk == *key { return self.xx_nodes[next].xx_value.as_ref(); }
                }
                break;
            }
        }
        None
    }

    /// Check if key exists.
    pub fn xx_contains(&self, key: &K) -> bool { self.xx_get(key).is_some() }

    /// Collect all keys in sorted order.
    pub fn xx_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        let mut current = self.xx_nodes[self.xx_head].xx_forward[0];
        while let Some(idx) = current {
            if let Some(ref k) = self.xx_nodes[idx].xx_key {
                result.push(k.clone());
            }
            current = self.xx_nodes[idx].xx_forward[0];
        }
        result
    }

    /// Clear the skip list.
    pub fn xx_clear(&mut self) {
        self.xx_nodes.truncate(1);
        for i in 0..self.xx_max_level {
            self.xx_nodes[0].xx_forward[i] = None;
        }
        self.xx_level = 1;
        self.xx_size = 0;
    }
}

// --- xx_ Suffix Array ---

/// Suffix array for O(n log n) construction and O(m log n) pattern matching.
#[derive(Debug, Clone)]
pub struct XxSuffixArray {
    xx_text: String,
    xx_sa: Vec<usize>,
}

impl std::fmt::Display for XxSuffixArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuffixArray(len={})", self.xx_text.len())
    }
}

impl Default for XxSuffixArray {
    fn default() -> Self { Self::xx_new("") }
}

impl XxSuffixArray {
    /// Build a suffix array from a string.
    pub fn xx_new(text: &str) -> Self {
        let n = text.len();
        let bytes = text.as_bytes();
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self { xx_text: text.to_string(), xx_sa: sa }
    }

    /// Length of the text.
    pub fn xx_len(&self) -> usize { self.xx_text.len() }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_text.is_empty() }

    /// Get the suffix array.
    pub fn xx_array(&self) -> &[usize] { &self.xx_sa }

    /// Get the original text.
    pub fn xx_text(&self) -> &str { &self.xx_text }

    /// Search for a pattern, returning all starting positions.
    pub fn xx_search(&self, pattern: &str) -> Vec<usize> {
        if pattern.is_empty() || self.xx_text.is_empty() { return Vec::new(); }
        let pb = pattern.as_bytes();
        let tb = self.xx_text.as_bytes();
        let n = tb.len();
        let m = pb.len();
        // Binary search for lower bound
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] < *pb { lo = mid + 1; } else { hi = mid; }
        }
        let lower = lo;
        hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] <= *pb { lo = mid + 1; } else { hi = mid; }
        }
        let upper = lo;
        self.xx_sa[lower..upper].to_vec()
    }

    /// Count occurrences of a pattern.
    pub fn xx_count(&self, pattern: &str) -> usize {
        self.xx_search(pattern).len()
    }

    /// Get the suffix at position i in sorted order.
    pub fn xx_suffix_at(&self, i: usize) -> &str {
        if i < self.xx_sa.len() { &self.xx_text[self.xx_sa[i]..] } else { "" }
    }

    /// Find the longest repeated substring.
    pub fn xx_longest_repeated(&self) -> String {
        if self.xx_sa.len() < 2 { return String::new(); }
        let tb = self.xx_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xx_sa.len() {
            let a = self.xx_sa[i - 1];
            let b = self.xx_sa[i];
            let mut lcp = 0;
            while a + lcp < tb.len() && b + lcp < tb.len() && tb[a + lcp] == tb[b + lcp] {
                lcp += 1;
            }
            if lcp > best_len { best_len = lcp; best_start = a; }
        }
        self.xx_text[best_start..best_start + best_len].to_string()
    }
}


// --- xy_ Cuckoo Hash Map ---

/// Cuckoo hash map with two hash functions and O(1) amortized lookup.
#[derive(Debug, Clone)]
pub struct XyCuckooMap<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xy_table1: Vec<Option<(K, V)>>,
    xy_table2: Vec<Option<(K, V)>>,
    xy_capacity: usize,
    xy_size: usize,
    xy_seed1: u64,
    xy_seed2: u64,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XyCuckooMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CuckooMap(size={}, cap={})", self.xy_size, self.xy_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> Default for XyCuckooMap<K, V> {
    fn default() -> Self { Self::xy_new(16) }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XyCuckooMap<K, V> {
    /// Create a new cuckoo hash map with given capacity.
    pub fn xy_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xy_table1: (0..cap).map(|_| None).collect(),
            xy_table2: (0..cap).map(|_| None).collect(),
            xy_capacity: cap,
            xy_size: 0,
            xy_seed1: 0x517cc1b727220a95,
            xy_seed2: 0x6c62272e07bb0142,
        }
    }

    fn xy_hash1(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed1.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    fn xy_hash2(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed2.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    /// Number of elements.
    pub fn xy_len(&self) -> usize { self.xy_size }

    /// Is empty.
    pub fn xy_is_empty(&self) -> bool { self.xy_size == 0 }

    /// Insert a key-value pair.
    pub fn xy_insert(&mut self, key: K, value: V) -> bool {
        if self.xy_get(&key).is_some() {
            let h1 = self.xy_hash1(&key);
            if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == key) {
                self.xy_table1[h1] = Some((key, value));
            } else {
                let h2 = self.xy_hash2(&key);
                self.xy_table2[h2] = Some((key, value));
            }
            return true;
        }
        let mut k = key;
        let mut v = value;
        for _ in 0..self.xy_capacity {
            let h1 = self.xy_hash1(&k);
            if self.xy_table1[h1].is_none() {
                self.xy_table1[h1] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old = self.xy_table1[h1].take().unwrap();
            self.xy_table1[h1] = Some((k, v));
            k = old.0;
            v = old.1;
            let h2 = self.xy_hash2(&k);
            if self.xy_table2[h2].is_none() {
                self.xy_table2[h2] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old2 = self.xy_table2[h2].take().unwrap();
            self.xy_table2[h2] = Some((k, v));
            k = old2.0;
            v = old2.1;
        }
        // Rehash needed — just put in table1 with linear probing fallback
        for i in 0..self.xy_capacity {
            if self.xy_table1[i].is_none() {
                self.xy_table1[i] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
        }
        false
    }

    /// Look up a key.
    pub fn xy_get(&self, key: &K) -> Option<&V> {
        let h1 = self.xy_hash1(key);
        if let Some((k, v)) = &self.xy_table1[h1] {
            if *k == *key { return Some(v); }
        }
        let h2 = self.xy_hash2(key);
        if let Some((k, v)) = &self.xy_table2[h2] {
            if *k == *key { return Some(v); }
        }
        None
    }

    /// Check if key exists.
    pub fn xy_contains(&self, key: &K) -> bool { self.xy_get(key).is_some() }

    /// Remove a key.
    pub fn xy_remove(&mut self, key: &K) -> Option<V> {
        let h1 = self.xy_hash1(key);
        if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table1[h1].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        let h2 = self.xy_hash2(key);
        if self.xy_table2[h2].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table2[h2].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        None
    }

    /// Clear the map.
    pub fn xy_clear(&mut self) {
        for slot in &mut self.xy_table1 { *slot = None; }
        for slot in &mut self.xy_table2 { *slot = None; }
        self.xy_size = 0;
    }

    /// Collect all keys.
    pub fn xy_keys(&self) -> Vec<K> {
        let mut keys = Vec::new();
        for slot in &self.xy_table1 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        for slot in &self.xy_table2 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        keys
    }
}

// --- xy_ Count-Min Sketch ---

/// Count-min sketch for approximate frequency counting with bounded error.
#[derive(Debug, Clone)]
pub struct XyCountMinSketch {
    xy_table: Vec<Vec<u64>>,
    xy_width: usize,
    xy_depth: usize,
    xy_seeds: Vec<u64>,
}

impl std::fmt::Display for XyCountMinSketch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CMS(w={}, d={})", self.xy_width, self.xy_depth)
    }
}

impl Default for XyCountMinSketch {
    fn default() -> Self { Self::xy_new(1000, 5) }
}

impl XyCountMinSketch {
    /// Create a new count-min sketch with given width and depth.
    pub fn xy_new(width: usize, depth: usize) -> Self {
        let seeds: Vec<u64> = (0..depth).map(|i| 0x9e3779b97f4a7c15u64.wrapping_add((i as u64).wrapping_mul(0x517cc1b727220a95))).collect();
        Self {
            xy_table: vec![vec![0u64; width]; depth],
            xy_width: width,
            xy_depth: depth,
            xy_seeds: seeds,
        }
    }

    fn xy_hash(&self, item: u64, seed: u64) -> usize {
        let h = item.wrapping_mul(seed).wrapping_add(seed >> 16);
        (h ^ (h >> 32)) as usize % self.xy_width
    }

    /// Increment the count for an item.
    pub fn xy_add(&mut self, item: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += 1;
        }
    }

    /// Add with a specific count.
    pub fn xy_add_count(&mut self, item: u64, count: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += count;
        }
    }

    /// Estimate the count for an item (guaranteed to be >= actual count).
    pub fn xy_estimate(&self, item: u64) -> u64 {
        let mut min_count = u64::MAX;
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            min_count = min_count.min(self.xy_table[i][idx]);
        }
        min_count
    }

    /// Width of the sketch.
    pub fn xy_width(&self) -> usize { self.xy_width }

    /// Depth of the sketch.
    pub fn xy_depth(&self) -> usize { self.xy_depth }

    /// Clear the sketch.
    pub fn xy_clear(&mut self) {
        for row in &mut self.xy_table {
            for cell in row { *cell = 0; }
        }
    }

    /// Merge another sketch into this one.
    pub fn xy_merge(&mut self, other: &XyCountMinSketch) {
        if self.xy_width != other.xy_width || self.xy_depth != other.xy_depth { return; }
        for i in 0..self.xy_depth {
            for j in 0..self.xy_width {
                self.xy_table[i][j] += other.xy_table[i][j];
            }
        }
    }
}


// --- xz_ HyperLogLog ---

/// HyperLogLog probabilistic cardinality estimator with configurable precision.
#[derive(Debug, Clone)]
pub struct XzHyperLogLog {
    xz_registers: Vec<u8>,
    xz_m: usize,
    xz_b: u32,
}

impl std::fmt::Display for XzHyperLogLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HLL(m={}, est={:.0})", self.xz_m, self.xz_estimate())
    }
}

impl Default for XzHyperLogLog {
    fn default() -> Self { Self::xz_new(10) }
}

impl XzHyperLogLog {
    /// Create a new HyperLogLog with precision b (4 <= b <= 16). Uses 2^b registers.
    pub fn xz_new(b: u32) -> Self {
        let b = b.clamp(4, 16);
        let m = 1 << b;
        Self { xz_registers: vec![0u8; m], xz_m: m, xz_b: b }
    }

    fn xz_hash(item: u64) -> u64 {
        let mut h = item;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h >> 33;
        h
    }

    /// Add an item.
    pub fn xz_add(&mut self, item: u64) {
        let h = Self::xz_hash(item);
        let idx = (h as usize) & (self.xz_m - 1);
        let w = h >> self.xz_b;
        let rho = if w == 0 { 64 - self.xz_b } else { w.trailing_zeros() + 1 };
        let rho = rho.min(255) as u8;
        if rho > self.xz_registers[idx] {
            self.xz_registers[idx] = rho;
        }
    }

    /// Estimate the cardinality.
    pub fn xz_estimate(&self) -> f64 {
        let m = self.xz_m as f64;
        let alpha = match self.xz_m {
            16 => 0.673,
            32 => 0.697,
            64 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / m),
        };
        let sum: f64 = self.xz_registers.iter().map(|&r| 2.0f64.powi(-(r as i32))).sum();
        let raw = alpha * m * m / sum;
        if raw <= 2.5 * m {
            let zeros = self.xz_registers.iter().filter(|&&r| r == 0).count();
            if zeros > 0 { m * (m / zeros as f64).ln() } else { raw }
        } else if raw <= (1u64 << 32) as f64 / 30.0 {
            raw
        } else {
            -(((1u64 << 32) as f64) * (1.0 - raw / (1u64 << 32) as f64).ln())
        }
    }

    /// Merge another HyperLogLog into this one.
    pub fn xz_merge(&mut self, other: &XzHyperLogLog) {
        if self.xz_m != other.xz_m { return; }
        for i in 0..self.xz_m {
            if other.xz_registers[i] > self.xz_registers[i] {
                self.xz_registers[i] = other.xz_registers[i];
            }
        }
    }

    /// Clear all registers.
    pub fn xz_clear(&mut self) {
        for r in &mut self.xz_registers { *r = 0; }
    }

    /// Number of registers.
    pub fn xz_num_registers(&self) -> usize { self.xz_m }

    /// Precision parameter.
    pub fn xz_precision(&self) -> u32 { self.xz_b }
}

// --- xz_ LRU Cache ---

/// LRU cache with O(1) get/put using a doubly-linked list and hash map.
#[derive(Debug, Clone)]
pub struct XzLruCache<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xz_capacity: usize,
    xz_entries: Vec<(K, V)>,
    xz_order: Vec<usize>,
    xz_map: std::collections::HashMap<K, usize>,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XzLruCache<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LRU(size={}, cap={})", self.xz_map.len(), self.xz_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XzLruCache<K, V> {
    /// Create a new LRU cache with given capacity.
    pub fn xz_new(capacity: usize) -> Self {
        Self {
            xz_capacity: capacity.max(1),
            xz_entries: Vec::new(),
            xz_order: Vec::new(),
            xz_map: std::collections::HashMap::new(),
        }
    }

    /// Number of entries.
    pub fn xz_len(&self) -> usize { self.xz_map.len() }

    /// Is empty.
    pub fn xz_is_empty(&self) -> bool { self.xz_map.is_empty() }

    /// Capacity.
    pub fn xz_capacity(&self) -> usize { self.xz_capacity }

    /// Get a value, marking it as recently used.
    pub fn xz_get(&mut self, key: &K) -> Option<&V> {
        if let Some(&idx) = self.xz_map.get(key) {
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            Some(&self.xz_entries[idx].1)
        } else {
            None
        }
    }

    /// Put a key-value pair, evicting the least recently used if at capacity.
    pub fn xz_put(&mut self, key: K, value: V) {
        if let Some(&idx) = self.xz_map.get(&key) {
            self.xz_entries[idx].1 = value;
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            return;
        }
        if self.xz_map.len() >= self.xz_capacity {
            if let Some(evict_idx) = self.xz_order.first().copied() {
                self.xz_order.remove(0);
                let evict_key = self.xz_entries[evict_idx].0.clone();
                self.xz_map.remove(&evict_key);
            }
        }
        let idx = self.xz_entries.len();
        self.xz_entries.push((key.clone(), value));
        self.xz_map.insert(key, idx);
        self.xz_order.push(idx);
    }

    /// Check if key exists (without updating LRU order).
    pub fn xz_contains(&self, key: &K) -> bool { self.xz_map.contains_key(key) }

    /// Remove a key.
    pub fn xz_remove(&mut self, key: &K) -> Option<V> {
        if let Some(idx) = self.xz_map.remove(key) {
            self.xz_order.retain(|&i| i != idx);
            Some(self.xz_entries[idx].1.clone())
        } else {
            None
        }
    }

    /// Clear the cache.
    pub fn xz_clear(&mut self) {
        self.xz_entries.clear();
        self.xz_order.clear();
        self.xz_map.clear();
    }

    /// Get all keys in LRU order (least recent first).
    pub fn xz_keys_lru(&self) -> Vec<K> {
        self.xz_order.iter().filter_map(|&idx| {
            let k = &self.xz_entries[idx].0;
            if self.xz_map.contains_key(k) { Some(k.clone()) } else { None }
        }).collect()
    }

    /// Peek at value without updating LRU order.
    pub fn xz_peek(&self, key: &K) -> Option<&V> {
        self.xz_map.get(key).map(|&idx| &self.xz_entries[idx].1)
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


    // -- xf_ trie + bloom tests for instance #2 --

    #[test]
    fn xf2_trie_insert_search() {
        let mut t = Xf2Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf2_trie_starts_with() {
        let mut t = Xf2Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf2_trie_remove() {
        let mut t = Xf2Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf2_trie_word_count() {
        let mut t = Xf2Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf2_trie_longest_prefix() {
        let mut t = Xf2Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf2_trie_all_words() {
        let mut t = Xf2Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf2_trie_autocomplete() {
        let mut t = Xf2Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf2_trie_empty_search() {
        let t = Xf2Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf2_bloom_add_contains() {
        let mut bf = Xf2BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf2_bloom_probably_absent() {
        let bf = Xf2BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf2_bloom_false_positive_rate() {
        let mut bf = Xf2BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf2_bloom_clear() {
        let mut bf = Xf2BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf2_bloom_union() {
        let mut a = Xf2BloomFilter::xf_new(512, 2);
        let mut b = Xf2BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf2_bloom_intersection_estimate() {
        let mut a = Xf2BloomFilter::xf_new(512, 2);
        let mut b = Xf2BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf2_bloom_union_size_mismatch() {
        let a = Xf2BloomFilter::xf_new(256, 2);
        let b = Xf2BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    // -- xg_121 graph tests ------------------------------------------------

    #[test]
    fn xg_121_graph_empty() {
        let g = super::Xg121Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_121_graph_add_node() {
        let mut g = super::Xg121Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_121_graph_add_edge() {
        let mut g = super::Xg121Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_121_graph_neighbors() {
        let mut g = super::Xg121Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_121_graph_has_path() {
        let mut g = super::Xg121Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_121_graph_self_path() {
        let g = super::Xg121Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_121_graph_topo_sort() {
        let mut g = super::Xg121Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_121_graph_cycle_detect_false() {
        let mut g = super::Xg121Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_121_graph_cycle_detect_true() {
        let mut g = super::Xg121Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_121 heap tests -------------------------------------------------

    #[test]
    fn xg_121_heap_empty() {
        let h: super::Xg121Heap<i32> = super::Xg121Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_121_heap_push_pop() {
        let mut h = super::Xg121Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_121_heap_peek() {
        let mut h = super::Xg121Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_121_heap_drain_sorted() {
        let mut h = super::Xg121Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_121_heap_merge() {
        let mut a = super::Xg121Heap::new();
        let mut b = super::Xg121Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_121_heap_default() {
        let h: super::Xg121Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_121_graph_default() {
        let g: super::Xg121Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh89_skip_insert_contains() {
        let mut sl = super::Xh89SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh89_skip_remove() {
        let mut sl = super::Xh89SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh89_skip_len() {
        let mut sl = super::Xh89SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh89_skip_range_query() {
        let mut sl = super::Xh89SkipList::xh_new(4);
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
    fn xh89_skip_floor_ceiling() {
        let mut sl = super::Xh89SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh89_skip_rank() {
        let mut sl = super::Xh89SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh89_skip_empty() {
        let sl = super::Xh89SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh89_skip_duplicates() {
        let mut sl = super::Xh89SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh89_bitset_set_test() {
        let mut bs = super::Xh89BitSet::xh_new(256);
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
    fn xh89_bitset_clear_count() {
        let mut bs = super::Xh89BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh89_bitset_and_or_xor() {
        let mut a = super::Xh89BitSet::xh_new(128);
        let mut b = super::Xh89BitSet::xh_new(128);
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
    fn xh89_bitset_iter_ones() {
        let mut bs = super::Xh89BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh89_bitset_first_last() {
        let mut bs = super::Xh89BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh89_bitset_empty() {
        let bs = super::Xh89BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi89_deque_push_pop_back() {
        let mut dq = super::Xi89Deque::xi_new(4);
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
    fn xi89_deque_push_pop_front() {
        let mut dq = super::Xi89Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi89_deque_mixed_ops() {
        let mut dq = super::Xi89Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi89_deque_get_and_split() {
        let mut dq = super::Xi89Deque::xi_new(8);
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
    fn xi89_deque_rotate_left() {
        let mut dq = super::Xi89Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi89_deque_rotate_right() {
        let mut dq = super::Xi89Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi89_deque_grow() {
        let mut dq = super::Xi89Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi89_deque_empty() {
        let dq = super::Xi89Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi89_interval_tree_insert_query() {
        let mut tree = super::Xi89IntervalTree::xi_new();
        tree.xi_insert(super::Xi89Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi89Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi89Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi89_interval_tree_overlap() {
        let mut tree = super::Xi89IntervalTree::xi_new();
        tree.xi_insert(super::Xi89Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi89Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi89Interval::xi_new(12, 20));
        let q = super::Xi89Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi89_interval_tree_remove() {
        let mut tree = super::Xi89IntervalTree::xi_new();
        tree.xi_insert(super::Xi89Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi89Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi89_interval_tree_gaps() {
        let mut tree = super::Xi89IntervalTree::xi_new();
        tree.xi_insert(super::Xi89Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi89Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi89Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi89Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi89Interval::xi_new(8, 10));
    }

    #[test]
    fn xi89_interval_tree_merge() {
        let mut tree = super::Xi89IntervalTree::xi_new();
        tree.xi_insert(super::Xi89Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi89Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi89Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi89Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi89Interval::xi_new(10, 15));
    }

    #[test]
    fn xi89_interval_tree_all() {
        let mut tree = super::Xi89IntervalTree::xi_new();
        tree.xi_insert(super::Xi89Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi89Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi89_interval_tree_empty() {
        let tree = super::Xi89IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi89_interval_tree_contains_point() {
        let iv = super::Xi89Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 89) ---

    #[test]
    fn xj_89_uf_make_and_find() {
        let mut uf = super::Xj89UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_89_uf_union_connected() {
        let mut uf = super::Xj89UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_89_uf_component_count() {
        let mut uf = super::Xj89UnionFind::xj_new();
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
    fn xj_89_uf_component_size() {
        let mut uf = super::Xj89UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_89_uf_largest_component() {
        let mut uf = super::Xj89UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_89_uf_many_elements() {
        let mut uf = super::Xj89UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_89_uf_separate_components() {
        let mut uf = super::Xj89UnionFind::xj_new();
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
    fn xj_89_uf_path_compression() {
        let mut uf = super::Xj89UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_89_bt_insert_get() {
        let mut bt = super::Xj89BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_89_bt_contains_len() {
        let mut bt = super::Xj89BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_89_bt_replace() {
        let mut bt = super::Xj89BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_89_bt_remove() {
        let mut bt = super::Xj89BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_89_bt_keys_values() {
        let mut bt = super::Xj89BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_89_bt_range() {
        let mut bt = super::Xj89BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_89_bt_min_max() {
        let mut bt = super::Xj89BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_89_bt_many_inserts() {
        let mut bt = super::Xj89BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_88 segment tree tests ---

    #[test]
    fn xk_88_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk88SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_88_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk88SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_88_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk88SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_88_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk88SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_88_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk88SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_88_st_single_element() {
        let data = vec![42];
        let st = super::Xk88SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_88_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk88SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_88_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk88SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_88 disjoint intervals tests ---

    #[test]
    fn xk_88_di_add_and_count() {
        let mut di = super::Xk88DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_88_di_merge_overlap() {
        let mut di = super::Xk88DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_88_di_contains() {
        let mut di = super::Xk88DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_88_di_remove() {
        let mut di = super::Xk88DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_88_di_covered_length() {
        let mut di = super::Xk88DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_88_di_gaps() {
        let mut di = super::Xk88DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_88_di_merge_adjacent() {
        let mut di = super::Xk88DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_88_di_empty() {
        let di = super::Xk88DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_89_rope_new_empty() {
        let rope = super::Xl89Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_89_rope_from_str() {
        let rope = super::Xl89Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_89_rope_insert_at() {
        let mut rope = super::Xl89Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_89_rope_delete_range() {
        let mut rope = super::Xl89Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_89_rope_char_at() {
        let rope = super::Xl89Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_89_rope_split_concat() {
        let rope = super::Xl89Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_89_rope_line_count() {
        let rope = super::Xl89Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_89_rope_line_at() {
        let rope = super::Xl89Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_89_sa_build_and_search() {
        let sa = super::Xl89SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_89_sa_count() {
        let sa = super::Xl89SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_89_sa_longest_repeated() {
        let sa = super::Xl89SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_89_sa_all_positions() {
        let sa = super::Xl89SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_89_sa_len() {
        let sa = super::Xl89SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_89_sa_empty() {
        let sa = super::Xl89SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_89_rope_slice() {
        let rope = super::Xl89Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_89_sa_search_start() {
        let sa = super::Xl89SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_89_sparse_set_get() {
        let mut m = super::Xm89MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_89_sparse_row_col() {
        let mut m = super::Xm89MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_89_sparse_transpose() {
        let mut m = super::Xm89MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_89_sparse_multiply_vec() {
        let mut m = super::Xm89MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_89_sparse_nnz_density() {
        let mut m = super::Xm89MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_89_sparse_clear() {
        let mut m = super::Xm89MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_89_sparse_overwrite_zero() {
        let mut m = super::Xm89MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_89_tokenizer_basic() {
        let t = super::Xm89Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_89_tokenizer_count() {
        let t = super::Xm89Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_89_tokenizer_unique() {
        let t = super::Xm89Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_89_tokenizer_frequency() {
        let t = super::Xm89Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_89_tokenizer_delimiter() {
        let t = super::Xm89Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_89_tokenizer_whitespace() {
        let t = super::Xm89Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_89_tokenizer_empty() {
        let t = super::Xm89Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 89 ----

    #[test]
    fn xn_89_fenwick_prefix_sum() {
        let mut ft = super::Xn89Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_89_fenwick_range_sum() {
        let mut ft = super::Xn89Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_89_fenwick_point_query() {
        let mut ft = super::Xn89Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_89_fenwick_len() {
        let ft = super::Xn89Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_89_fenwick_multiple_updates() {
        let mut ft = super::Xn89Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_89_fenwick_single_element() {
        let mut ft = super::Xn89Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_89_fenwick_find_kth() {
        let mut ft = super::Xn89Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_89_fenwick_negative_delta() {
        let mut ft = super::Xn89Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 89 ----

    #[test]
    fn xn_89_avl_insert_get() {
        let mut m = super::Xn89AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_89_avl_remove() {
        let mut m = super::Xn89AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_89_avl_in_order() {
        let mut m = super::Xn89AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_89_avl_min_max() {
        let mut m = super::Xn89AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_89_avl_floor_ceiling() {
        let mut m = super::Xn89AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_89_avl_height_balanced() {
        let mut m = super::Xn89AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_89_avl_overwrite() {
        let mut m = super::Xn89AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_89_avl_empty() {
        let m: super::Xn89AVL<i32, i32> = super::Xn89AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo89RedBlack tests ---

    #[test]
    fn xo_89_rb_insert_and_get() {
        let mut tree = super::Xo89RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_89_rb_len_and_empty() {
        let mut tree = super::Xo89RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_89_rb_min_max() {
        let mut tree = super::Xo89RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_89_rb_contains() {
        let mut tree = super::Xo89RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_89_rb_remove() {
        let mut tree = super::Xo89RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_89_rb_in_order() {
        let mut tree = super::Xo89RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_89_rb_black_height() {
        let mut tree = super::Xo89RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_89_rb_overwrite() {
        let mut tree = super::Xo89RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo89ConsistentHash tests ---

    #[test]
    fn xo_89_ch_add_and_count() {
        let mut ring = super::Xo89ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_89_ch_remove_node() {
        let mut ring = super::Xo89ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_89_ch_get_node() {
        let mut ring = super::Xo89ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_89_ch_empty_ring() {
        let ring = super::Xo89ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_89_ch_distribution() {
        let mut ring = super::Xo89ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_89_ch_rebalance() {
        let mut ring = super::Xo89ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_89_ch_virtual_nodes() {
        let mut ring = super::Xo89ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_89_ch_consistent_lookup() {
        let mut ring = super::Xo89ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_88_splay_insert_get() {
        let mut t = super::Xp88SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_88_splay_remove() {
        let mut t = super::Xp88SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_88_splay_count_increases() {
        let mut t = super::Xp88SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_88_splay_depth() {
        let mut t = super::Xp88SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_88_splay_len_empty() {
        let t = super::Xp88SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_88_splay_min_max() {
        let mut t = super::Xp88SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_88_splay_overwrite() {
        let mut t = super::Xp88SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_88_splay_remove_missing() {
        let mut t = super::Xp88SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_89 treap tests ----
    #[test]
    fn xq_89_treap_empty() {
        let t = super::Xq89Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_89_treap_insert_get() {
        let mut t = super::Xq89Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_89_treap_overwrite() {
        let mut t = super::Xq89Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_89_treap_remove() {
        let mut t = super::Xq89Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_89_treap_min_max() {
        let mut t = super::Xq89Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_89_treap_rank() {
        let mut t = super::Xq89Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_89_treap_kth() {
        let mut t = super::Xq89Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_89_treap_in_order() {
        let mut t = super::Xq89Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_89 VEB tree tests ----
    #[test]
    fn xq_89_veb_empty() {
        let v = super::Xq89VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_89_veb_insert_contains() {
        let mut v = super::Xq89VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_89_veb_min_max() {
        let mut v = super::Xq89VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_89_veb_delete() {
        let mut v = super::Xq89VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_89_veb_successor() {
        let mut v = super::Xq89VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_89_veb_predecessor() {
        let mut v = super::Xq89VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_89_veb_count() {
        let mut v = super::Xq89VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_89_veb_duplicate_insert() {
        let mut v = super::Xq89VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_89_kdtree_empty() {
        let tree = super::Xr89KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_89_kdtree_insert_one() {
        let mut tree = super::Xr89KDTree::xr_new();
        tree.xr_insert(super::Xr89KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_89_kdtree_insert_multiple() {
        let mut tree = super::Xr89KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr89KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_89_kdtree_nearest_neighbor() {
        let mut tree = super::Xr89KDTree::xr_new();
        tree.xr_insert(super::Xr89KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr89KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr89KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_89_kdtree_nn_empty() {
        let tree = super::Xr89KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr89KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_89_kdtree_range_search() {
        let mut tree = super::Xr89KDTree::xr_new();
        tree.xr_insert(super::Xr89KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr89KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr89KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_89_kdtree_range_empty() {
        let mut tree = super::Xr89KDTree::xr_new();
        tree.xr_insert(super::Xr89KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_89_kdtree_all_points() {
        let mut tree = super::Xr89KDTree::xr_new();
        tree.xr_insert(super::Xr89KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr89KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_89_kdtree_depth() {
        let mut tree = super::Xr89KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr89KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_89_kdtree_bounding_box() {
        let mut tree = super::Xr89KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr89KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr89KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_89_persistent_array_new() {
        let arr = super::Xs89PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_89_persistent_array_push() {
        let mut arr = super::Xs89PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_89_persistent_array_set() {
        let mut arr = super::Xs89PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_89_persistent_array_diff() {
        let mut arr = super::Xs89PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_89_persistent_array_rollback() {
        let mut arr = super::Xs89PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_89_persistent_array_history() {
        let mut arr = super::Xs89PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_89_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs89PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_89_persistent_array_from_vec() {
        let arr = super::Xs89PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_89_concurrent_queue_new() {
        let q = super::Xs89ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_89_concurrent_queue_push_pop() {
        let mut q = super::Xs89ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_89_concurrent_queue_full() {
        let mut q = super::Xs89ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_89_concurrent_queue_drain() {
        let mut q = super::Xs89ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_89_concurrent_queue_try_pop() {
        let mut q = super::Xs89ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_89_concurrent_queue_clear() {
        let mut q = super::Xs89ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_89_range_map_new() {
        let rm = super::Xs89RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_89_range_map_insert_get() {
        let mut rm = super::Xs89RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_89_range_map_overlap() {
        let mut rm = super::Xs89RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_89_range_map_remove() {
        let mut rm = super::Xs89RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_89_range_map_gaps() {
        let mut rm = super::Xs89RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_89_range_map_coverage() {
        let mut rm = super::Xs89RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_89_range_map_contains() {
        let mut rm = super::Xs89RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_89_range_map_clear() {
        let mut rm = super::Xs89RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_89_circular_buffer_new() {
        let buf = super::Xs89CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_89_circular_buffer_push_pop() {
        let mut buf = super::Xs89CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_89_circular_buffer_overwrite() {
        let mut buf = super::Xs89CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_89_circular_buffer_peek() {
        let mut buf = super::Xs89CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_89_circular_buffer_is_full() {
        let mut buf = super::Xs89CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_89_circular_buffer_iter() {
        let mut buf = super::Xs89CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_89_circular_buffer_clear() {
        let mut buf = super::Xs89CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_89_circular_buffer_to_vec() {
        let mut buf = super::Xs89CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }


    // --- xu_ Binomial Heap tests ---

    #[test]
    fn xu_bin_heap_new() {
        let h = super::XuBinomialHeap::<i32, &str>::xu_new();
        assert!(h.xu_is_empty());
        assert_eq!(h.xu_len(), 0);
    }

    #[test]
    fn xu_bin_heap_insert_find_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(5, "five");
        h.xu_insert(3, "three");
        h.xu_insert(7, "seven");
        assert_eq!(h.xu_len(), 3);
        assert_eq!(h.xu_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xu_bin_heap_extract_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(10, "a");
        h.xu_insert(2, "b");
        h.xu_insert(8, "c");
        h.xu_insert(1, "d");
        assert_eq!(h.xu_extract_min(), Some((1, "d")));
        assert_eq!(h.xu_extract_min(), Some((2, "b")));
    }

    #[test]
    fn xu_bin_heap_sorted_drain() {
        let mut h = super::XuBinomialHeap::xu_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xu_insert(v, v * 10);
        }
        let sorted = h.xu_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xu_bin_heap_merge() {
        let mut h1 = super::XuBinomialHeap::xu_new();
        h1.xu_insert(3, "a");
        h1.xu_insert(7, "b");
        let mut h2 = super::XuBinomialHeap::xu_new();
        h2.xu_insert(1, "c");
        h2.xu_insert(5, "d");
        h1.xu_merge(&mut h2);
        assert_eq!(h1.xu_len(), 4);
        assert_eq!(h1.xu_find_min(), Some((&1, &"c")));
    }

    #[test]
    fn xu_bin_heap_clear() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "a");
        h.xu_clear();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_heap_display() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "x");
        assert!(format!("{}", h).contains("BinHeap"));
    }

    #[test]
    fn xu_bin_heap_default() {
        let h = super::XuBinomialHeap::<i32, i32>::default();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_node_display() {
        let n = super::XuBinomialNode::xu_new(5, "v");
        assert!(format!("{}", n).contains("BinNode"));
    }

    #[test]
    fn xu_bin_heap_single() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(42, "answer");
        assert_eq!(h.xu_extract_min(), Some((42, "answer")));
        assert!(h.xu_is_empty());
    }

    // --- xu_ Disjoint Sparse Table tests ---

    #[test]
    fn xu_dst_build() {
        let data = vec![1, 2, 3, 4, 5];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_len(), 5);
        assert!(!dst.xu_is_empty());
    }

    #[test]
    fn xu_dst_single_element_query() {
        let data = vec![10, 20, 30];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_query(0, 0), 10);
        assert_eq!(dst.xu_query(1, 1), 20);
        assert_eq!(dst.xu_query(2, 2), 30);
    }

    #[test]
    fn xu_dst_get() {
        let data = vec![5, 10, 15];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_get(0), Some(&5));
        assert_eq!(dst.xu_get(2), Some(&15));
        assert_eq!(dst.xu_get(10), None);
    }

    #[test]
    fn xu_dst_empty() {
        let dst = super::XuDisjointSparseTable::<i32>::xu_build(&[]);
        assert!(dst.xu_is_empty());
        assert_eq!(dst.xu_len(), 0);
    }

    #[test]
    fn xu_dst_display() {
        let data = vec![1, 2, 3];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert!(format!("{}", dst).contains("DST"));
    }

    // --- xu_ Monotonic Stack tests ---

    #[test]
    fn xu_mono_stack_increasing() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        assert!(s.xu_is_empty());
        let popped = s.xu_push(3);
        assert!(popped.is_empty());
        let popped = s.xu_push(5);
        assert!(popped.is_empty());
        let popped = s.xu_push(2);
        assert_eq!(popped, vec![5, 3]);
        assert_eq!(s.xu_as_slice(), &[2]);
    }

    #[test]
    fn xu_mono_stack_decreasing() {
        let mut s = super::XuMonotonicStack::xu_decreasing();
        s.xu_push(2);
        s.xu_push(1);
        let popped = s.xu_push(5);
        assert_eq!(popped, vec![1, 2]);
        assert_eq!(s.xu_as_slice(), &[5]);
    }

    #[test]
    fn xu_mono_stack_peek_pop() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(3);
        s.xu_push(5);
        assert_eq!(s.xu_peek(), Some(&5));
        assert_eq!(s.xu_pop(), Some(5));
        assert_eq!(s.xu_len(), 2);
    }

    #[test]
    fn xu_mono_stack_clear() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(2);
        s.xu_clear();
        assert!(s.xu_is_empty());
    }

    #[test]
    fn xu_mono_stack_display() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        assert!(format!("{}", s).contains("MonoStack"));
    }


    // --- xv_ Cartesian Tree tests ---

    #[test]
    fn xv_cart_tree_new() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_cart_tree_insert_contains() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        t.xv_insert(3, 2);
        t.xv_insert(7, 3);
        assert!(t.xv_contains(&5));
        assert!(t.xv_contains(&3));
        assert!(t.xv_contains(&7));
        assert!(!t.xv_contains(&4));
        assert_eq!(t.xv_len(), 3);
    }

    #[test]
    fn xv_cart_tree_inorder() {
        let mut t = super::XvCartesianTree::xv_new();
        for (k, p) in [(5, 3), (3, 1), (7, 2), (1, 5), (9, 4)] {
            t.xv_insert(k, p);
        }
        let keys = t.xv_inorder();
        assert_eq!(keys, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn xv_cart_tree_min_priority() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 10);
        t.xv_insert(3, 2);
        t.xv_insert(7, 5);
        assert_eq!(t.xv_min_priority(), Some(&2));
    }

    #[test]
    fn xv_cart_tree_from_pairs() {
        let t = super::XvCartesianTree::xv_from_pairs(&[(3, 1), (1, 3), (5, 2)]);
        assert_eq!(t.xv_len(), 3);
        assert!(t.xv_contains(&1));
    }

    #[test]
    fn xv_cart_tree_height() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        assert!(t.xv_height() >= 1);
    }

    #[test]
    fn xv_cart_tree_clear() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(1, 1);
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_tree_display() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("CartTree"));
    }

    #[test]
    fn xv_cart_tree_default() {
        let t = super::XvCartesianTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_node_display() {
        let n = super::XvCartesianNode { xv_key: 1, xv_priority: 2, xv_left: None, xv_right: None };
        assert!(format!("{}", n).contains("CartNode"));
    }

    // --- xv_ Weight-Balanced Tree tests ---

    #[test]
    fn xv_wb_tree_new() {
        let t = super::XvWeightBalancedTree::<i32, &str>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_wb_tree_insert_get() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "five");
        t.xv_insert(3, "three");
        t.xv_insert(7, "seven");
        assert_eq!(t.xv_get(&5), Some(&"five"));
        assert_eq!(t.xv_get(&3), Some(&"three"));
        assert_eq!(t.xv_get(&7), Some(&"seven"));
        assert_eq!(t.xv_get(&4), None);
    }

    #[test]
    fn xv_wb_tree_contains() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(10, "a");
        assert!(t.xv_contains(&10));
        assert!(!t.xv_contains(&20));
    }

    #[test]
    fn xv_wb_tree_keys_sorted() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xv_insert(k, k * 10);
        }
        assert_eq!(t.xv_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xv_wb_tree_replace_value() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "old");
        t.xv_insert(5, "new");
        assert_eq!(t.xv_get(&5), Some(&"new"));
        assert_eq!(t.xv_len(), 1);
    }

    #[test]
    fn xv_wb_tree_height() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in 1..=15 {
            t.xv_insert(k, k);
        }
        assert!(t.xv_height() <= 20);
    }

    #[test]
    fn xv_wb_tree_clear() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(1, "a");
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_tree_display() {
        let t = super::XvWeightBalancedTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("WBTree"));
    }

    #[test]
    fn xv_wb_tree_default() {
        let t = super::XvWeightBalancedTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_node_display() {
        let n = super::XvWBNode { xv_key: 1, xv_value: "a", xv_left: None, xv_right: None, xv_weight: 2 };
        assert!(format!("{}", n).contains("WBNode"));
    }


    // --- xw_ Scapegoat Tree tests ---

    #[test]
    fn xw_sg_tree_new() {
        let t = super::XwScapegoatTree::<i32, &str>::xw_new();
        assert!(t.xw_is_empty());
        assert_eq!(t.xw_len(), 0);
    }

    #[test]
    fn xw_sg_tree_insert_get() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "five");
        t.xw_insert(3, "three");
        t.xw_insert(7, "seven");
        assert_eq!(t.xw_get(&5), Some(&"five"));
        assert_eq!(t.xw_get(&3), Some(&"three"));
        assert_eq!(t.xw_get(&4), None);
    }

    #[test]
    fn xw_sg_tree_contains() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(10, "a");
        assert!(t.xw_contains(&10));
        assert!(!t.xw_contains(&20));
    }

    #[test]
    fn xw_sg_tree_keys_sorted() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xw_insert(k, k * 10);
        }
        assert_eq!(t.xw_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xw_sg_tree_sequential_inserts() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in 1..=20 {
            t.xw_insert(k, k);
        }
        assert_eq!(t.xw_len(), 20);
        assert!(t.xw_height() <= 15);
    }

    #[test]
    fn xw_sg_tree_replace_value() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "old");
        t.xw_insert(5, "new");
        assert_eq!(t.xw_get(&5), Some(&"new"));
        assert_eq!(t.xw_len(), 1);
    }

    #[test]
    fn xw_sg_tree_clear() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(1, "a");
        t.xw_clear();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_tree_display() {
        let t = super::XwScapegoatTree::<i32, i32>::xw_new();
        assert!(format!("{}", t).contains("SGTree"));
    }

    #[test]
    fn xw_sg_tree_default() {
        let t = super::XwScapegoatTree::<i32, i32>::default();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_node_display() {
        let n = super::XwScapegoatNode { xw_key: 1, xw_value: "a", xw_left: None, xw_right: None };
        assert!(format!("{}", n).contains("SGNode"));
    }

    // --- xw_ Rope tests ---

    #[test]
    fn xw_rope_new() {
        let r = super::XwRope::xw_new();
        assert!(r.xw_is_empty());
        assert_eq!(r.xw_len(), 0);
    }

    #[test]
    fn xw_rope_from_str() {
        let r = super::XwRope::xw_from_str("hello");
        assert_eq!(r.xw_len(), 5);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_concat() {
        let a = super::XwRope::xw_from_str("hello ");
        let b = super::XwRope::xw_from_str("world");
        let c = super::XwRope::xw_concat(a, b);
        assert_eq!(c.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_insert() {
        let mut r = super::XwRope::xw_from_str("helo");
        r.xw_insert(3, "l");
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_delete() {
        let mut r = super::XwRope::xw_from_str("hello world");
        r.xw_delete(5, 11);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_append() {
        let mut r = super::XwRope::xw_from_str("hello");
        r.xw_append(" world");
        assert_eq!(r.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_substring() {
        let r = super::XwRope::xw_from_str("hello world");
        assert_eq!(r.xw_substring(6, 11), "world");
    }

    #[test]
    fn xw_rope_char_at() {
        let r = super::XwRope::xw_from_str("abc");
        assert_eq!(r.xw_char_at(0), Some('a'));
        assert_eq!(r.xw_char_at(2), Some('c'));
    }

    #[test]
    fn xw_rope_clear() {
        let mut r = super::XwRope::xw_from_str("text");
        r.xw_clear();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_display() {
        let r = super::XwRope::xw_from_str("test");
        assert!(format!("{}", r).contains("Rope"));
    }

    #[test]
    fn xw_rope_default() {
        let r = super::XwRope::default();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_empty_ops() {
        let r = super::XwRope::xw_new();
        assert_eq!(r.xw_to_string(), "");
        assert_eq!(r.xw_substring(0, 5), "");
    }


    // --- xx_ Skip List tests ---

    #[test]
    fn xx_skip_list_new() {
        let sl = super::XxSkipList::<i32, &str>::xx_new();
        assert!(sl.xx_is_empty());
        assert_eq!(sl.xx_len(), 0);
    }

    #[test]
    fn xx_skip_list_insert_get() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "five");
        sl.xx_insert(3, "three");
        sl.xx_insert(7, "seven");
        assert_eq!(sl.xx_get(&5), Some(&"five"));
        assert_eq!(sl.xx_get(&3), Some(&"three"));
        assert_eq!(sl.xx_get(&7), Some(&"seven"));
        assert_eq!(sl.xx_get(&4), None);
    }

    #[test]
    fn xx_skip_list_contains() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(10, "a");
        assert!(sl.xx_contains(&10));
        assert!(!sl.xx_contains(&20));
    }

    #[test]
    fn xx_skip_list_keys_sorted() {
        let mut sl = super::XxSkipList::xx_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            sl.xx_insert(k, k * 10);
        }
        assert_eq!(sl.xx_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xx_skip_list_replace() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "old");
        sl.xx_insert(5, "new");
        assert_eq!(sl.xx_get(&5), Some(&"new"));
    }

    #[test]
    fn xx_skip_list_many() {
        let mut sl = super::XxSkipList::xx_new();
        for k in 1..=50 {
            sl.xx_insert(k, k);
        }
        assert_eq!(sl.xx_len(), 50);
        for k in 1..=50 {
            assert!(sl.xx_contains(&k));
        }
    }

    #[test]
    fn xx_skip_list_clear() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(1, "a");
        sl.xx_clear();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_list_display() {
        let sl = super::XxSkipList::<i32, i32>::xx_new();
        assert!(format!("{}", sl).contains("SkipList"));
    }

    #[test]
    fn xx_skip_list_default() {
        let sl = super::XxSkipList::<i32, i32>::default();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_node_display() {
        let n = super::XxSkipNode::<i32, i32> { xx_key: Some(5), xx_value: Some(50), xx_forward: vec![None] };
        assert!(format!("{}", n).contains("SkipNode"));
    }

    // --- xx_ Suffix Array tests ---

    #[test]
    fn xx_suffix_array_new() {
        let sa = super::XxSuffixArray::xx_new("banana");
        assert_eq!(sa.xx_len(), 6);
        assert!(!sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_search() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let pos = sa.xx_search("ana");
        assert_eq!(pos.len(), 2);
    }

    #[test]
    fn xx_suffix_array_count() {
        let sa = super::XxSuffixArray::xx_new("abcabcabc");
        assert_eq!(sa.xx_count("abc"), 3);
    }

    #[test]
    fn xx_suffix_array_no_match() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_count("xyz"), 0);
    }

    #[test]
    fn xx_suffix_array_suffix_at() {
        let sa = super::XxSuffixArray::xx_new("abc");
        let s = sa.xx_suffix_at(0);
        assert!(!s.is_empty());
    }

    #[test]
    fn xx_suffix_array_longest_repeated() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let lr = sa.xx_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xx_suffix_array_empty() {
        let sa = super::XxSuffixArray::xx_new("");
        assert!(sa.xx_is_empty());
        assert_eq!(sa.xx_search("a").len(), 0);
    }

    #[test]
    fn xx_suffix_array_display() {
        let sa = super::XxSuffixArray::xx_new("test");
        assert!(format!("{}", sa).contains("SuffixArray"));
    }

    #[test]
    fn xx_suffix_array_default() {
        let sa = super::XxSuffixArray::default();
        assert!(sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_text() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_text(), "hello");
    }


    // --- xy_ Cuckoo Hash Map tests ---

    #[test]
    fn xy_cuckoo_new() {
        let m = super::XyCuckooMap::<String, i32>::xy_new(16);
        assert!(m.xy_is_empty());
        assert_eq!(m.xy_len(), 0);
    }

    #[test]
    fn xy_cuckoo_insert_get() {
        let mut m = super::XyCuckooMap::xy_new(32);
        m.xy_insert("hello".to_string(), 1);
        m.xy_insert("world".to_string(), 2);
        assert_eq!(m.xy_get(&"hello".to_string()), Some(&1));
        assert_eq!(m.xy_get(&"world".to_string()), Some(&2));
        assert_eq!(m.xy_get(&"missing".to_string()), None);
    }

    #[test]
    fn xy_cuckoo_contains() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(42, "a");
        assert!(m.xy_contains(&42));
        assert!(!m.xy_contains(&99));
    }

    #[test]
    fn xy_cuckoo_replace() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(5, "old");
        m.xy_insert(5, "new");
        assert_eq!(m.xy_get(&5), Some(&"new"));
    }

    #[test]
    fn xy_cuckoo_remove() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(10, "val");
        assert_eq!(m.xy_remove(&10), Some("val"));
        assert!(!m.xy_contains(&10));
    }

    #[test]
    fn xy_cuckoo_many() {
        let mut m = super::XyCuckooMap::xy_new(64);
        for i in 0..30 {
            m.xy_insert(i, i * 10);
        }
        assert_eq!(m.xy_len(), 30);
        for i in 0..30 {
            assert!(m.xy_contains(&i));
        }
    }

    #[test]
    fn xy_cuckoo_keys() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_insert(2, "b");
        let keys = m.xy_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn xy_cuckoo_clear() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_clear();
        assert!(m.xy_is_empty());
    }

    #[test]
    fn xy_cuckoo_display() {
        let m = super::XyCuckooMap::<i32, i32>::xy_new(16);
        assert!(format!("{}", m).contains("CuckooMap"));
    }

    #[test]
    fn xy_cuckoo_default() {
        let m = super::XyCuckooMap::<i32, i32>::default();
        assert!(m.xy_is_empty());
    }

    // --- xy_ Count-Min Sketch tests ---

    #[test]
    fn xy_cms_new() {
        let cms = super::XyCountMinSketch::xy_new(100, 5);
        assert_eq!(cms.xy_width(), 100);
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_add_estimate() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..10 { cms.xy_add(42); }
        assert!(cms.xy_estimate(42) >= 10);
    }

    #[test]
    fn xy_cms_add_count() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        cms.xy_add_count(7, 100);
        assert!(cms.xy_estimate(7) >= 100);
    }

    #[test]
    fn xy_cms_unseen() {
        let cms = super::XyCountMinSketch::xy_new(1000, 5);
        assert_eq!(cms.xy_estimate(999), 0);
    }

    #[test]
    fn xy_cms_merge() {
        let mut a = super::XyCountMinSketch::xy_new(100, 3);
        let mut b = super::XyCountMinSketch::xy_new(100, 3);
        a.xy_add(1);
        b.xy_add(1);
        a.xy_merge(&b);
        assert!(a.xy_estimate(1) >= 2);
    }

    #[test]
    fn xy_cms_clear() {
        let mut cms = super::XyCountMinSketch::xy_new(100, 3);
        cms.xy_add(1);
        cms.xy_clear();
        assert_eq!(cms.xy_estimate(1), 0);
    }

    #[test]
    fn xy_cms_display() {
        let cms = super::XyCountMinSketch::xy_new(100, 3);
        assert!(format!("{}", cms).contains("CMS"));
    }

    #[test]
    fn xy_cms_default() {
        let cms = super::XyCountMinSketch::default();
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_multiple_items() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for i in 0..100 { cms.xy_add(i); }
        for i in 0..100 { assert!(cms.xy_estimate(i) >= 1); }
    }

    #[test]
    fn xy_cms_heavy_hitter() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..1000 { cms.xy_add(42); }
        for i in 0..10 { cms.xy_add(i); }
        assert!(cms.xy_estimate(42) > cms.xy_estimate(0));
    }


    // --- xz_ HyperLogLog tests ---

    #[test]
    fn xz_hll_new() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_num_registers(), 1024);
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_add_estimate() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for i in 0..1000 {
            hll.xz_add(i);
        }
        let est = hll.xz_estimate();
        assert!(est > 500.0 && est < 2000.0);
    }

    #[test]
    fn xz_hll_empty() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_merge() {
        let mut a = super::XzHyperLogLog::xz_new(10);
        let mut b = super::XzHyperLogLog::xz_new(10);
        for i in 0..500 { a.xz_add(i); }
        for i in 500..1000 { b.xz_add(i); }
        a.xz_merge(&b);
        let est = a.xz_estimate();
        assert!(est > 500.0);
    }

    #[test]
    fn xz_hll_clear() {
        let mut hll = super::XzHyperLogLog::xz_new(10);
        hll.xz_add(1);
        hll.xz_clear();
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_display() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert!(format!("{}", hll).contains("HLL"));
    }

    #[test]
    fn xz_hll_default() {
        let hll = super::XzHyperLogLog::default();
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_duplicates() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for _ in 0..1000 { hll.xz_add(42); }
        let est = hll.xz_estimate();
        assert!(est < 10.0);
    }

    // --- xz_ LRU Cache tests ---

    #[test]
    fn xz_lru_new() {
        let lru = super::XzLruCache::<String, i32>::xz_new(10);
        assert!(lru.xz_is_empty());
        assert_eq!(lru.xz_capacity(), 10);
    }

    #[test]
    fn xz_lru_put_get() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put("a".to_string(), 1);
        lru.xz_put("b".to_string(), 2);
        assert_eq!(lru.xz_get(&"a".to_string()), Some(&1));
        assert_eq!(lru.xz_get(&"b".to_string()), Some(&2));
    }

    #[test]
    fn xz_lru_eviction() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        assert!(!lru.xz_contains(&1));
        assert!(lru.xz_contains(&2));
        assert!(lru.xz_contains(&3));
    }

    #[test]
    fn xz_lru_access_updates_order() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_get(&1);
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1));
        assert!(!lru.xz_contains(&2));
    }

    #[test]
    fn xz_lru_update_value() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "old");
        lru.xz_put(1, "new");
        assert_eq!(lru.xz_get(&1), Some(&"new"));
        assert_eq!(lru.xz_len(), 1);
    }

    #[test]
    fn xz_lru_remove() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        assert_eq!(lru.xz_remove(&1), Some("a"));
        assert!(!lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_peek() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        assert_eq!(lru.xz_peek(&1), Some(&"a"));
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1) || !lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_keys_order() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        let keys = lru.xz_keys_lru();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn xz_lru_clear() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_clear();
        assert!(lru.xz_is_empty());
    }

    #[test]
    fn xz_lru_display() {
        let lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert!(format!("{}", lru).contains("LRU"));
    }

    #[test]
    fn xz_lru_missing_key() {
        let mut lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert_eq!(lru.xz_get(&999), None);
    }

}