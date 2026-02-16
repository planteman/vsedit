//! Hover tooltip service.
//!
//! Equivalent to VS Code's `vs/editor/contrib/hover`.
//! Provides hover content model for displaying tooltips at cursor positions.

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
}
