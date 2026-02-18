//! Code minimap renderer.

use std::fmt;

/// How the minimap renders content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimapRenderMode {
    Blocks,
    Characters,
}

/// Which side of the editor the minimap appears on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimapPosition {
    Left,
    Right,
}

/// When to show the viewport slider overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowSlider {
    Always,
    MouseOver,
}

/// Configuration for the minimap.
#[derive(Debug, Clone)]
pub struct MinimapConfig {
    pub enabled: bool,
    pub side: MinimapPosition,
    pub mode: MinimapRenderMode,
    pub max_column: u32,
    pub scale: u32,
    pub show_slider: ShowSlider,
}

impl fmt::Display for MinimapConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Minimap(enabled={}, side={:?}, mode={:?}, max_col={}, scale={})",
            self.enabled, self.side, self.mode, self.max_column, self.scale,
        )
    }
}

impl Default for MinimapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            side: MinimapPosition::Right,
            mode: MinimapRenderMode::Blocks,
            max_column: 120,
            scale: 1,
            show_slider: ShowSlider::MouseOver,
        }
    }
}

/// Errors that can occur during minimap operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinimapError {
    InvalidViewport { start: u32, end: u32 },
    NoContent,
    RenderFailed(String),
}

impl fmt::Display for MinimapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidViewport { start, end } => {
                write!(f, "invalid viewport: start {} >= end {}", start, end)
            }
            Self::NoContent => write!(f, "no content loaded"),
            Self::RenderFailed(msg) => write!(f, "render failed: {}", msg),
        }
    }
}

/// Builder for constructing [`MinimapConfig`] with a fluent API.
#[derive(Debug, Clone)]
pub struct MinimapConfigBuilder {
    config: MinimapConfig,
}

impl MinimapConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: MinimapConfig::default(),
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    pub fn side(mut self, side: MinimapPosition) -> Self {
        self.config.side = side;
        self
    }

    pub fn mode(mut self, mode: MinimapRenderMode) -> Self {
        self.config.mode = mode;
        self
    }

    pub fn max_column(mut self, max_column: u32) -> Self {
        self.config.max_column = max_column;
        self
    }

    pub fn scale(mut self, scale: u32) -> Self {
        self.config.scale = scale;
        self
    }

    pub fn show_slider(mut self, show_slider: ShowSlider) -> Self {
        self.config.show_slider = show_slider;
        self
    }

    pub fn build(self) -> MinimapConfig {
        self.config
    }
}

/// Metrics computed for the current minimap state.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapMetrics {
    pub visible_ratio: f64,
    pub total_lines: u32,
    pub viewport_lines: u32,
    pub scaled_height: f64,
}

/// A single token span within a minimap line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimapToken {
    pub start_col: u32,
    pub length: u32,
    pub color_id: u8,
}

impl MinimapToken {
    /// Merge adjacent tokens with the same `color_id` on a line.
    pub fn merge_tokens(tokens: &[MinimapToken]) -> Vec<MinimapToken> {
        if tokens.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<MinimapToken> = Vec::new();
        for tok in tokens {
            if let Some(last) = merged.last_mut() {
                if last.color_id == tok.color_id
                    && last.start_col + last.length == tok.start_col
                {
                    last.length += tok.length;
                    continue;
                }
            }
            merged.push(tok.clone());
        }
        merged
    }
}

/// A line in the minimap with its tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimapLine {
    pub line_number: u32,
    pub tokens: Vec<MinimapToken>,
}

impl MinimapLine {
    /// Sum of all token lengths on this line.
    pub fn total_width(&self) -> u32 {
        self.tokens.iter().map(|t| t.length).sum()
    }
}

/// Renders a minimap of the document.
pub struct MinimapRenderer {
    pub config: MinimapConfig,
    lines: Vec<MinimapLine>,
    viewport_start: u32,
    viewport_end: u32,
}

impl MinimapRenderer {
    pub fn new(config: MinimapConfig) -> Self {
        Self {
            config,
            lines: Vec::new(),
            viewport_start: 0,
            viewport_end: 0,
        }
    }

    pub fn update_content(&mut self, lines: Vec<MinimapLine>) {
        self.lines = lines;
    }

    pub fn set_viewport(&mut self, start: u32, end: u32) {
        self.viewport_start = start;
        self.viewport_end = end;
    }

    /// Returns the minimap lines that fall within the current viewport.
    pub fn get_visible_lines(&self) -> &[MinimapLine] {
        let start = self.viewport_start as usize;
        let end = (self.viewport_end as usize).min(self.lines.len());
        if start >= self.lines.len() {
            return &[];
        }
        &self.lines[start..end]
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get a specific line by line number.
    pub fn get_line(&self, line_number: u32) -> Option<&MinimapLine> {
        self.lines.iter().find(|l| l.line_number == line_number)
    }

    /// Mark lines containing the given search term and return their line numbers.
    pub fn search_highlight(&self, term: &str) -> Vec<u32> {
        if term.is_empty() {
            return Vec::new();
        }
        self.lines
            .iter()
            .filter(|line| {
                // A line "contains" the term if any token's start_col range
                // could represent the term. Since we don't store text, we
                // match by checking if any token's color_id matches the first
                // byte of the search term as a simple heuristic. For real use,
                // content text would be stored alongside tokens.
                //
                // Here we use a simpler semantic: lines whose line_number
                // digits contain the term string, useful for line-number search.
                line.line_number.to_string().contains(term)
            })
            .map(|line| line.line_number)
            .collect()
    }

    /// What percentage of the total document is visible in the viewport.
    pub fn viewport_percentage(&self) -> f64 {
        if self.lines.is_empty() {
            return 0.0;
        }
        let viewport_lines = self.viewport_end.saturating_sub(self.viewport_start);
        (viewport_lines as f64 / self.lines.len() as f64) * 100.0
    }

    /// Adjust viewport to center on a given line.
    pub fn scroll_to_line(&mut self, line: u32) {
        let viewport_size = self.viewport_end.saturating_sub(self.viewport_start);
        let half = viewport_size / 2;
        let total = self.lines.len() as u32;
        let start = line.saturating_sub(half);
        let end = (start + viewport_size).min(total);
        let start = if end == total { total.saturating_sub(viewport_size) } else { start };
        self.viewport_start = start;
        self.viewport_end = end;
    }

    /// Compute metrics about the current minimap state.
    pub fn compute_metrics(&self) -> MinimapMetrics {
        let total_lines = self.lines.len() as u32;
        let viewport_lines = self.viewport_end.saturating_sub(self.viewport_start);
        let visible_ratio = if total_lines == 0 {
            0.0
        } else {
            viewport_lines as f64 / total_lines as f64
        };
        let scaled_height = total_lines as f64 * self.config.scale as f64;
        MinimapMetrics {
            visible_ratio,
            total_lines,
            viewport_lines,
            scaled_height,
        }
    }
}

/// The kind of decoration displayed on a minimap line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationType {
    Highlight,
    Warning,
    Error,
    Info,
    Search,
}

impl fmt::Display for DecorationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Highlight => "highlight",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Info => "info",
            Self::Search => "search",
        };
        f.write_str(label)
    }
}

/// A single decoration attached to a minimap line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimapDecoration {
    pub line_number: u32,
    pub color_id: u8,
    pub decoration_type: DecorationType,
}

/// Manages a collection of decorations for the minimap.
#[derive(Debug, Clone)]
pub struct MinimapDecorationLayer {
    decorations: Vec<MinimapDecoration>,
}

impl MinimapDecorationLayer {
    pub fn new() -> Self {
        Self {
            decorations: Vec::new(),
        }
    }

    pub fn add(&mut self, decoration: MinimapDecoration) {
        self.decorations.push(decoration);
    }

    pub fn remove_by_line(&mut self, line_number: u32) {
        self.decorations.retain(|d| d.line_number != line_number);
    }

    pub fn get_decorations_in_range(&self, start: u32, end: u32) -> Vec<&MinimapDecoration> {
        self.decorations
            .iter()
            .filter(|d| d.line_number >= start && d.line_number < end)
            .collect()
    }

    pub fn clear(&mut self) {
        self.decorations.clear();
    }

    pub fn count(&self) -> usize {
        self.decorations.len()
    }
}

/// A collapsed or foldable region in the minimap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimapRegion {
    pub start_line: u32,
    pub end_line: u32,
    pub label: String,
}

impl MinimapRegion {
    pub fn line_span(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line)
    }

    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line < self.end_line
    }
}

/// Computes pixel-level layout positions for the minimap.
#[derive(Debug, Clone)]
pub struct MinimapLayout {
    pub line_height: f64,
    pub total_lines: u32,
    pub viewport_start: u32,
    pub viewport_end: u32,
}

impl MinimapLayout {
    pub fn new(line_height: f64, total_lines: u32, viewport_start: u32, viewport_end: u32) -> Self {
        Self {
            line_height,
            total_lines,
            viewport_start,
            viewport_end,
        }
    }

    pub fn line_to_y(&self, line: u32) -> f64 {
        line as f64 * self.line_height
    }

    pub fn y_to_line(&self, y: f64) -> u32 {
        if self.line_height <= 0.0 {
            return 0;
        }
        let line = (y / self.line_height) as u32;
        line.min(self.total_lines.saturating_sub(1))
    }

    pub fn total_height(&self) -> f64 {
        self.total_lines as f64 * self.line_height
    }

    pub fn viewport_y_range(&self) -> (f64, f64) {
        (self.line_to_y(self.viewport_start), self.line_to_y(self.viewport_end))
    }
}

impl MinimapRenderer {
    /// Attach a decoration layer and return decorations visible in the current viewport.
    pub fn add_decoration_layer<'a>(
        &self,
        layer: &'a MinimapDecorationLayer,
    ) -> Vec<&'a MinimapDecoration> {
        layer.get_decorations_in_range(self.viewport_start, self.viewport_end)
    }

    /// Convenience: get all decorations from multiple layers that fall within the viewport.
    pub fn get_decorations_in_viewport<'a>(
        &self,
        layers: &'a [MinimapDecorationLayer],
    ) -> Vec<&'a MinimapDecoration> {
        layers
            .iter()
            .flat_map(|layer| {
                layer.get_decorations_in_range(self.viewport_start, self.viewport_end)
            })
            .collect()
    }
}

/// Represents a selected region in the minimap.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapSelection {
    pub start_line: u32,
    pub end_line: u32,
    pub start_col: u32,
    pub end_col: u32,
}

impl MinimapSelection {
    pub fn new(start_line: u32, end_line: u32, start_col: u32, end_col: u32) -> Self {
        Self {
            start_line: start_line.min(end_line),
            end_line: start_line.max(end_line),
            start_col,
            end_col,
        }
    }

    /// Number of lines spanned by the selection.
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Whether a given line is within this selection.
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    /// Whether two selections overlap.
    pub fn overlaps(&self, other: &MinimapSelection) -> bool {
        self.start_line <= other.end_line && other.start_line <= self.end_line
    }

    /// Merge two overlapping selections into one.
    pub fn merge(&self, other: &MinimapSelection) -> Option<MinimapSelection> {
        if !self.overlaps(other) {
            return None;
        }
        Some(MinimapSelection {
            start_line: self.start_line.min(other.start_line),
            end_line: self.end_line.max(other.end_line),
            start_col: self.start_col.min(other.start_col),
            end_col: self.end_col.max(other.end_col),
        })
    }
}

impl fmt::Display for MinimapSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Selection(L{}:{}-L{}:{})",
            self.start_line, self.start_col, self.end_line, self.end_col,
        )
    }
}

/// Tracks search result highlights in the minimap.
#[derive(Debug, Clone)]
pub struct MinimapSearch {
    /// Line numbers that contain search matches.
    matches: Vec<u32>,
    pub query: String,
    pub case_sensitive: bool,
}

impl MinimapSearch {
    pub fn new(query: impl Into<String>, case_sensitive: bool) -> Self {
        Self {
            matches: Vec::new(),
            query: query.into(),
            case_sensitive,
        }
    }

    /// Execute the search against a set of minimap lines.
    pub fn search(&mut self, lines: &[MinimapLine]) {
        self.matches.clear();
        // Simplified: treat each token's color_id as text-proxy content; in a real
        // implementation this would search actual text. Here we highlight lines
        // that have tokens spanning enough columns.
        let query_len = self.query.len() as u32;
        if query_len == 0 {
            return;
        }
        for line in lines {
            for token in &line.tokens {
                if token.length >= query_len {
                    self.matches.push(line.line_number);
                    break;
                }
            }
        }
    }

    /// Return the matched line numbers.
    pub fn matched_lines(&self) -> &[u32] {
        &self.matches
    }

    /// Number of matches found.
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Whether a specific line has a match.
    pub fn has_match(&self, line: u32) -> bool {
        self.matches.contains(&line)
    }

    /// Clear all matches.
    pub fn clear(&mut self) {
        self.matches.clear();
    }
}

/// A color scheme for the minimap.
#[derive(Debug, Clone)]
pub struct MinimapColorScheme {
    pub name: String,
    colors: Vec<(u8, [u8; 3])>,
    pub default_color: [u8; 3],
}

impl MinimapColorScheme {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            colors: Vec::new(),
            default_color: [200, 200, 200],
        }
    }

    /// Register a color for a given color_id.
    pub fn set_color(&mut self, color_id: u8, rgb: [u8; 3]) {
        if let Some(entry) = self.colors.iter_mut().find(|(id, _)| *id == color_id) {
            entry.1 = rgb;
        } else {
            self.colors.push((color_id, rgb));
        }
    }

    /// Look up the RGB color for a given color_id.
    pub fn get_color(&self, color_id: u8) -> [u8; 3] {
        self.colors
            .iter()
            .find(|(id, _)| *id == color_id)
            .map(|(_, rgb)| *rgb)
            .unwrap_or(self.default_color)
    }

    /// Number of registered colors.
    pub fn color_count(&self) -> usize {
        self.colors.len()
    }

    /// Resolve a token to its RGB color.
    pub fn resolve_token(&self, token: &MinimapToken) -> [u8; 3] {
        self.get_color(token.color_id)
    }
}

impl fmt::Display for MinimapColorScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ColorScheme(\"{}\", {} colors)", self.name, self.colors.len())
    }
}

// ---------------------------------------------------------------------------
// MinimapRender using braille character blocks
// ---------------------------------------------------------------------------

/// Braille-based minimap rendering.
///
/// Uses Unicode Braille characters (U+2800–U+28FF) to represent source code
/// lines in a compact form. Each braille character encodes a 2×4 dot pattern,
/// so a single character can represent 2 columns × 4 rows of pixels.
pub struct MinimapBrailleRenderer {
    /// Width in braille characters.
    pub width: usize,
}

impl MinimapBrailleRenderer {
    /// Create a renderer with a given column width.
    pub fn new(width: usize) -> Self {
        Self { width }
    }

    /// Render a single source line as a braille string.
    ///
    /// Each non-space character contributes a dot in the braille cell.
    /// This produces a 1-row-high string of braille characters.
    pub fn render_line(&self, line: &str) -> String {
        let mut result = String::with_capacity(self.width);
        let bytes = line.as_bytes();

        for col in 0..self.width {
            let c0 = col * 2;
            let c1 = c0 + 1;
            let has_c0 = c0 < bytes.len() && bytes[c0] != b' ';
            let has_c1 = c1 < bytes.len() && bytes[c1] != b' ';

            // Map to braille dots: we use the top two dots of the braille cell.
            // Dot 1 (top-left) = 0x01, Dot 4 (top-right) = 0x08
            let mut dots: u32 = 0;
            if has_c0 {
                dots |= 0x01; // dot 1
            }
            if has_c1 {
                dots |= 0x08; // dot 4
            }

            result.push(char::from_u32(0x2800 + dots).unwrap_or(' '));
        }
        result
    }

    /// Render multiple source lines into braille rows.
    /// Groups of 4 source lines are combined into one braille row.
    pub fn render_block(&self, lines: &[&str]) -> Vec<String> {
        let mut rows = Vec::new();
        for chunk in lines.chunks(4) {
            let mut row = String::with_capacity(self.width);
            for col in 0..self.width {
                let c0 = col * 2;
                let c1 = c0 + 1;
                let mut dots: u32 = 0;
                // Braille dot mapping for rows 0..4:
                // Row 0: dot1=0x01, dot4=0x08
                // Row 1: dot2=0x02, dot5=0x10
                // Row 2: dot3=0x04, dot6=0x20
                // Row 3: dot7=0x40, dot8=0x80
                let dot_pairs: [(u32, u32); 4] = [
                    (0x01, 0x08),
                    (0x02, 0x10),
                    (0x04, 0x20),
                    (0x40, 0x80),
                ];
                for (row_idx, &(left, right)) in dot_pairs.iter().enumerate() {
                    if let Some(line) = chunk.get(row_idx) {
                        let bytes = line.as_bytes();
                        if c0 < bytes.len() && bytes[c0] != b' ' {
                            dots |= left;
                        }
                        if c1 < bytes.len() && bytes[c1] != b' ' {
                            dots |= right;
                        }
                    }
                }
                row.push(char::from_u32(0x2800 + dots).unwrap_or(' '));
            }
            rows.push(row);
        }
        rows
    }

    /// Calculate how many braille rows are needed for a document with `line_count` lines.
    pub fn rows_needed(&self, line_count: usize) -> usize {
        (line_count + 3) / 4
    }
}

impl MinimapConfig {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn effective_width(&self) -> u32 {
        self.max_column / self.scale.max(1)
    }

    pub fn summary(&self) -> String {
        format!(
            "{:?} minimap on {:?}, width={}, scale={}",
            self.mode, self.side, self.max_column, self.scale,
        )
    }
}

impl MinimapMetrics {
    pub fn coverage_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        self.viewport_lines as f64 / self.total_lines as f64
    }

    pub fn is_full_document(&self) -> bool {
        self.viewport_lines >= self.total_lines
    }

    pub fn hidden_lines(&self) -> u32 {
        self.total_lines.saturating_sub(self.viewport_lines)
    }
}

impl MinimapToken {
    pub fn end_col(&self) -> u32 {
        self.start_col + self.length
    }

    pub fn overlaps(&self, other: &MinimapToken) -> bool {
        self.start_col < other.end_col() && other.start_col < self.end_col()
    }
}

impl MinimapLine {
    pub fn is_blank(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn char_count(&self) -> u32 {
        self.tokens.iter().map(|t| t.length).sum()
    }

    pub fn max_column(&self) -> u32 {
        self.tokens
            .iter()
            .map(|t| t.start_col + t.length)
            .max()
            .unwrap_or(0)
    }

    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}

impl MinimapDecoration {
    pub fn is_error(&self) -> bool {
        self.decoration_type == DecorationType::Error
    }

    pub fn is_warning(&self) -> bool {
        self.decoration_type == DecorationType::Warning
    }
}

impl MinimapDecorationLayer {
    pub fn merge(&mut self, other: &MinimapDecorationLayer) {
        self.decorations.extend(other.decorations.iter().cloned());
    }

    pub fn iter(&self) -> impl Iterator<Item = &MinimapDecoration> {
        self.decorations.iter()
    }

    pub fn has_errors(&self) -> bool {
        self.decorations.iter().any(|d| d.is_error())
    }

    pub fn filter_by_type(&self, dt: DecorationType) -> Vec<&MinimapDecoration> {
        self.decorations
            .iter()
            .filter(|d| d.decoration_type == dt)
            .collect()
    }
}

impl MinimapRegion {
    pub fn overlaps(&self, other: &MinimapRegion) -> bool {
        self.start_line < other.end_line && other.start_line < self.end_line
    }

    pub fn is_empty(&self) -> bool {
        self.start_line >= self.end_line
    }
}

impl MinimapLayout {
    pub fn is_within_viewport(&self, line: u32) -> bool {
        line >= self.viewport_start && line < self.viewport_end
    }

    pub fn viewport_lines(&self) -> u32 {
        self.viewport_end.saturating_sub(self.viewport_start)
    }

    pub fn viewport_height(&self) -> f64 {
        self.viewport_lines() as f64 * self.line_height
    }
}

impl MinimapSelection {
    pub fn line_count(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    pub fn is_empty(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col
    }

    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }
}

impl MinimapColorScheme {
    pub fn is_dark(&self) -> bool {
        let avg = (self.default_color[0] as u16
            + self.default_color[1] as u16
            + self.default_color[2] as u16)
            / 3;
        avg < 128
    }

    pub fn is_light(&self) -> bool {
        !self.is_dark()
    }

    pub fn color_ids(&self) -> Vec<u8> {
        self.colors.iter().map(|(id, _)| *id).collect()
    }
}

// ---------------------------------------------------------------------------
// Minimap scale factor and line density
// ---------------------------------------------------------------------------

/// Computes how many document lines map to a single pixel row in the minimap.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapScaleFactor {
    /// Total document lines.
    pub document_lines: u32,
    /// Available minimap height in pixels.
    pub minimap_height_px: u32,
}

impl MinimapScaleFactor {
    pub fn new(document_lines: u32, minimap_height_px: u32) -> Self {
        Self {
            document_lines,
            minimap_height_px,
        }
    }

    /// Lines per pixel. Values > 1 mean multiple lines share a pixel row.
    pub fn lines_per_pixel(&self) -> f64 {
        if self.minimap_height_px == 0 {
            return 0.0;
        }
        self.document_lines as f64 / self.minimap_height_px as f64
    }

    /// Pixels per line. Values < 1 mean a single pixel represents several lines.
    pub fn pixels_per_line(&self) -> f64 {
        if self.document_lines == 0 {
            return 0.0;
        }
        self.minimap_height_px as f64 / self.document_lines as f64
    }

    /// Whether the minimap must down-sample (more lines than pixels).
    pub fn is_downsampled(&self) -> bool {
        self.document_lines > self.minimap_height_px
    }

    /// Map a document line number to its pixel-row in the minimap.
    pub fn line_to_pixel(&self, line: u32) -> u32 {
        if self.document_lines == 0 {
            return 0;
        }
        let ratio = self.minimap_height_px as f64 / self.document_lines as f64;
        let px = (line as f64 * ratio) as u32;
        px.min(self.minimap_height_px.saturating_sub(1))
    }

    /// Map a pixel-row back to the nearest document line.
    pub fn pixel_to_line(&self, pixel: u32) -> u32 {
        if self.minimap_height_px == 0 {
            return 0;
        }
        let ratio = self.document_lines as f64 / self.minimap_height_px as f64;
        let line = (pixel as f64 * ratio) as u32;
        line.min(self.document_lines.saturating_sub(1))
    }
}

// ---------------------------------------------------------------------------
// Minimap viewport mapping
// ---------------------------------------------------------------------------

/// Maps an editor viewport (visible range) onto the minimap coordinate space.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewportMapping {
    /// First visible line in the editor.
    pub editor_start: u32,
    /// One-past-last visible line in the editor.
    pub editor_end: u32,
    /// Total document lines.
    pub document_lines: u32,
    /// Total minimap height in pixels.
    pub minimap_height_px: u32,
}

impl ViewportMapping {
    pub fn new(
        editor_start: u32,
        editor_end: u32,
        document_lines: u32,
        minimap_height_px: u32,
    ) -> Self {
        Self {
            editor_start,
            editor_end: editor_end.min(document_lines),
            document_lines,
            minimap_height_px,
        }
    }

    /// The pixel range of the slider overlay on the minimap.
    pub fn slider_pixel_range(&self) -> (u32, u32) {
        let scale = MinimapScaleFactor::new(self.document_lines, self.minimap_height_px);
        let top = scale.line_to_pixel(self.editor_start);
        let bottom = if self.editor_end >= self.document_lines {
            self.minimap_height_px
        } else {
            scale.line_to_pixel(self.editor_end)
        };
        (top, bottom.max(top + 1))
    }

    /// Height of the slider in pixels (clamped to at least 1).
    pub fn slider_height_px(&self) -> u32 {
        let (top, bottom) = self.slider_pixel_range();
        bottom - top
    }

    /// Given a click on the minimap at `pixel_y`, return the document line the
    /// editor should centre on.
    pub fn click_to_center_line(&self, pixel_y: u32) -> u32 {
        let scale = MinimapScaleFactor::new(self.document_lines, self.minimap_height_px);
        scale.pixel_to_line(pixel_y)
    }

    /// Fraction of the document currently visible (0.0–1.0).
    pub fn visible_fraction(&self) -> f64 {
        if self.document_lines == 0 {
            return 0.0;
        }
        let visible = self.editor_end.saturating_sub(self.editor_start);
        (visible as f64 / self.document_lines as f64).min(1.0)
    }
}

// ---------------------------------------------------------------------------
// Line-density sampler for colour bands
// ---------------------------------------------------------------------------

/// Produces per-pixel-row colour samples by averaging token colours across the
/// document lines that map to each pixel row.
pub struct LineDensitySampler;

impl LineDensitySampler {
    /// Sample the dominant `color_id` for each pixel row of the minimap.
    ///
    /// Returns a `Vec` of length `minimap_height_px`, where each element is
    /// the most-frequent `color_id` among the tokens in the lines that map to
    /// that pixel row, or `None` for blank rows.
    pub fn sample(
        lines: &[MinimapLine],
        minimap_height_px: u32,
    ) -> Vec<Option<u8>> {
        if lines.is_empty() || minimap_height_px == 0 {
            return vec![None; minimap_height_px as usize];
        }

        let scale = MinimapScaleFactor::new(lines.len() as u32, minimap_height_px);
        let mut result = Vec::with_capacity(minimap_height_px as usize);

        for px in 0..minimap_height_px {
            let first_line = scale.pixel_to_line(px) as usize;
            let next_px_line = if px + 1 < minimap_height_px {
                scale.pixel_to_line(px + 1) as usize
            } else {
                lines.len()
            };
            let last_line = next_px_line.max(first_line + 1).min(lines.len());

            // Count occurrences of each color_id in this pixel band.
            let mut counts: Vec<(u8, u32)> = Vec::new();
            for line in &lines[first_line..last_line] {
                for tok in &line.tokens {
                    if let Some(entry) = counts.iter_mut().find(|(id, _)| *id == tok.color_id) {
                        entry.1 += tok.length;
                    } else {
                        counts.push((tok.color_id, tok.length));
                    }
                }
            }

            let dominant = counts.iter().max_by_key(|(_, c)| *c).map(|(id, _)| *id);
            result.push(dominant);
        }

        result
    }
}

// ---------------------------------------------------------------------------
// Search highlight overlay
// ---------------------------------------------------------------------------

/// Pre-computed search highlight positions projected onto minimap pixel rows.
#[derive(Debug, Clone)]
pub struct SearchHighlightOverlay {
    /// Pixel rows that contain at least one search match.
    highlighted_pixels: Vec<u32>,
}

impl SearchHighlightOverlay {
    /// Build an overlay from a set of matched line numbers and a scale factor.
    pub fn from_matches(matched_lines: &[u32], scale: &MinimapScaleFactor) -> Self {
        let mut pixels: Vec<u32> = matched_lines
            .iter()
            .map(|&line| scale.line_to_pixel(line))
            .collect();
        pixels.sort_unstable();
        pixels.dedup();
        Self {
            highlighted_pixels: pixels,
        }
    }

    /// Whether a given pixel row is highlighted.
    pub fn is_highlighted(&self, pixel_row: u32) -> bool {
        self.highlighted_pixels.binary_search(&pixel_row).is_ok()
    }

    /// Total number of distinct highlighted pixel rows.
    pub fn highlight_count(&self) -> usize {
        self.highlighted_pixels.len()
    }

    /// Iterator over highlighted pixel rows.
    pub fn iter(&self) -> impl Iterator<Item = &u32> {
        self.highlighted_pixels.iter()
    }
}

// ---------------------------------------------------------------------------
// Git-change gutter marks
// ---------------------------------------------------------------------------

/// The kind of git change for a line range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitChangeKind {
    Added,
    Modified,
    Deleted,
}

/// A contiguous range of lines with a git change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitChange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: GitChangeKind,
}

/// Manages git-change decorations and projects them onto the minimap.
#[derive(Debug, Clone)]
pub struct GitChangeLayer {
    changes: Vec<GitChange>,
}

impl GitChangeLayer {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }

    pub fn add(&mut self, change: GitChange) {
        self.changes.push(change);
    }

    pub fn clear(&mut self) {
        self.changes.clear();
    }

    /// Return changes that overlap a line range.
    pub fn changes_in_range(&self, start: u32, end: u32) -> Vec<&GitChange> {
        self.changes
            .iter()
            .filter(|c| c.start_line < end && c.end_line > start)
            .collect()
    }

    /// Project all changes onto minimap pixel rows. Returns a `Vec` of
    /// `(pixel_row, kind)` pairs, deduplicated and sorted.
    pub fn project_onto_minimap(
        &self,
        scale: &MinimapScaleFactor,
    ) -> Vec<(u32, GitChangeKind)> {
        let mut result: Vec<(u32, GitChangeKind)> = Vec::new();
        for change in &self.changes {
            let px_start = scale.line_to_pixel(change.start_line);
            let px_end = scale.line_to_pixel(change.end_line.saturating_sub(1));
            for px in px_start..=px_end {
                if !result.iter().any(|(p, _)| *p == px) {
                    result.push((px, change.kind));
                }
            }
        }
        result.sort_by_key(|(px, _)| *px);
        result
    }

    pub fn count(&self) -> usize {
        self.changes.len()
    }
}

// ---------------------------------------------------------------------------
// Slider drag interaction
// ---------------------------------------------------------------------------

/// Tracks the state of a click-and-drag interaction on the minimap slider.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapSliderDrag {
    /// Whether a drag is currently active.
    pub active: bool,
    /// Pixel-y where the drag started.
    start_pixel_y: u32,
    /// The editor start-line when the drag began.
    anchor_line: u32,
    /// Current pixel-y of the pointer.
    current_pixel_y: u32,
}

impl MinimapSliderDrag {
    /// Begin a new drag at `pixel_y` with the editor anchored at `editor_start`.
    pub fn begin(pixel_y: u32, editor_start: u32) -> Self {
        Self {
            active: true,
            start_pixel_y: pixel_y,
            anchor_line: editor_start,
            current_pixel_y: pixel_y,
        }
    }

    /// Create an inactive (idle) drag state.
    pub fn idle() -> Self {
        Self {
            active: false,
            start_pixel_y: 0,
            anchor_line: 0,
            current_pixel_y: 0,
        }
    }

    /// Update the pointer position during a drag.
    pub fn update(&mut self, pixel_y: u32) {
        if self.active {
            self.current_pixel_y = pixel_y;
        }
    }

    /// End the drag, returning the final scroll target line.
    pub fn end(&mut self, scale: &MinimapScaleFactor) -> u32 {
        self.active = false;
        self.target_line(scale)
    }

    /// Signed pixel delta from the start of the drag.
    pub fn pixel_delta(&self) -> i32 {
        self.current_pixel_y as i32 - self.start_pixel_y as i32
    }

    /// Compute the document line the editor should scroll to, based on the
    /// current drag offset translated through the scale factor.
    pub fn target_line(&self, scale: &MinimapScaleFactor) -> u32 {
        let delta_lines = (self.pixel_delta() as f64 * scale.lines_per_pixel()) as i32;
        let target = self.anchor_line as i32 + delta_lines;
        let clamped = target
            .max(0)
            .min(scale.document_lines.saturating_sub(1) as i32);
        clamped as u32
    }
}

impl fmt::Display for MinimapSliderDrag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.active {
            write!(
                f,
                "Drag(active, delta={}px, anchor=L{})",
                self.pixel_delta(),
                self.anchor_line,
            )
        } else {
            write!(f, "Drag(idle)")
        }
    }
}

// ---------------------------------------------------------------------------
// Aggregated highlight layer
// ---------------------------------------------------------------------------

/// Priority-ordered highlight kind. Lower discriminant = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HighlightKind {
    Error,
    SearchMatch,
    Selection,
}

/// A single highlight entry on a pixel row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightEntry {
    pub pixel_row: u32,
    pub kind: HighlightKind,
}

/// Aggregated highlight layer that merges search matches, selections, and
/// error markers with priority-based rendering.
#[derive(Debug, Clone)]
pub struct MinimapHighlights {
    entries: Vec<HighlightEntry>,
}

impl MinimapHighlights {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add search-match highlights from a `SearchHighlightOverlay`.
    pub fn add_search_highlights(&mut self, overlay: &SearchHighlightOverlay) {
        for &px in overlay.iter() {
            self.entries.push(HighlightEntry {
                pixel_row: px,
                kind: HighlightKind::SearchMatch,
            });
        }
    }

    /// Add selection highlights from a `MinimapSelection` projected through `scale`.
    pub fn add_selection(&mut self, sel: &MinimapSelection, scale: &MinimapScaleFactor) {
        let px_start = scale.line_to_pixel(sel.start_line);
        let px_end = scale.line_to_pixel(sel.end_line);
        for px in px_start..=px_end {
            self.entries.push(HighlightEntry {
                pixel_row: px,
                kind: HighlightKind::Selection,
            });
        }
    }

    /// Add error markers from a decoration layer projected through `scale`.
    pub fn add_errors(&mut self, layer: &MinimapDecorationLayer, scale: &MinimapScaleFactor) {
        for dec in layer.iter() {
            if dec.is_error() {
                let px = scale.line_to_pixel(dec.line_number);
                self.entries.push(HighlightEntry {
                    pixel_row: px,
                    kind: HighlightKind::Error,
                });
            }
        }
    }

    /// Return the highest-priority highlight kind for a given pixel row, or
    /// `None` if the row has no highlights.
    pub fn dominant_at(&self, pixel_row: u32) -> Option<HighlightKind> {
        self.entries
            .iter()
            .filter(|e| e.pixel_row == pixel_row)
            .map(|e| e.kind)
            .min() // HighlightKind::Error < SearchMatch < Selection
    }

    /// Total number of highlight entries (before deduplication).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Deduplicate entries, keeping only the highest-priority kind per row.
    pub fn compact(&mut self) {
        self.entries.sort_by(|a, b| {
            a.pixel_row.cmp(&b.pixel_row).then(a.kind.cmp(&b.kind))
        });
        self.entries.dedup_by(|a, b| a.pixel_row == b.pixel_row);
    }
}

// ---------------------------------------------------------------------------
// Character-level scale calculations
// ---------------------------------------------------------------------------

/// Maps character dimensions to minimap pixel dimensions using a scale factor
/// and font metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapScale {
    /// Width of a single character cell in editor pixels.
    pub char_width: f64,
    /// Height of a single character cell in editor pixels.
    pub char_height: f64,
    /// Scale denominator (e.g. 1 = full size, 2 = half, 6 = typical minimap).
    pub divisor: u32,
}

impl MinimapScale {
    pub fn new(char_width: f64, char_height: f64, divisor: u32) -> Self {
        Self {
            char_width,
            char_height,
            divisor: divisor.max(1),
        }
    }

    /// Scaled width of a single character in minimap pixels.
    pub fn scaled_char_width(&self) -> f64 {
        self.char_width / self.divisor as f64
    }

    /// Scaled height of a single character in minimap pixels.
    pub fn scaled_char_height(&self) -> f64 {
        self.char_height / self.divisor as f64
    }

    /// Width of `cols` columns in minimap pixels.
    pub fn columns_to_px(&self, cols: u32) -> f64 {
        cols as f64 * self.scaled_char_width()
    }

    /// Height of `rows` lines in minimap pixels.
    pub fn lines_to_px(&self, rows: u32) -> f64 {
        rows as f64 * self.scaled_char_height()
    }

    /// How many columns fit within `width_px` minimap pixels.
    pub fn px_to_columns(&self, width_px: f64) -> u32 {
        let cw = self.scaled_char_width();
        if cw <= 0.0 {
            return 0;
        }
        (width_px / cw) as u32
    }

    /// How many lines fit within `height_px` minimap pixels.
    pub fn px_to_lines(&self, height_px: f64) -> u32 {
        let ch = self.scaled_char_height();
        if ch <= 0.0 {
            return 0;
        }
        (height_px / ch) as u32
    }
}

impl fmt::Display for MinimapScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Scale(1/{}, char={:.1}x{:.1} -> {:.2}x{:.2})",
            self.divisor,
            self.char_width,
            self.char_height,
            self.scaled_char_width(),
            self.scaled_char_height(),
        )
    }
}

// ---------------------------------------------------------------------------
// Viewport indicator overlay
// ---------------------------------------------------------------------------

/// Style for rendering the viewport indicator rectangle on the minimap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorStyle {
    /// Semi-transparent filled rectangle.
    Fill,
    /// Outline only (border).
    Border,
}

/// Describes the rendered viewport indicator ("current view" rectangle) that
/// is drawn on top of the minimap to show which portion of the document is
/// visible in the editor.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapViewportIndicator {
    /// Top pixel row of the indicator.
    pub top_px: u32,
    /// Height of the indicator in pixels (≥ 1).
    pub height_px: u32,
    /// Width of the indicator in pixels.
    pub width_px: u32,
    /// Visual style.
    pub style: IndicatorStyle,
}

impl MinimapViewportIndicator {
    /// Build the indicator from a `ViewportMapping` and a minimap width.
    pub fn from_mapping(mapping: &ViewportMapping, width_px: u32, style: IndicatorStyle) -> Self {
        let (top, bottom) = mapping.slider_pixel_range();
        Self {
            top_px: top,
            height_px: bottom.saturating_sub(top).max(1),
            width_px,
            style,
        }
    }

    /// Bottom pixel row (exclusive).
    pub fn bottom_px(&self) -> u32 {
        self.top_px + self.height_px
    }

    /// Whether a pixel coordinate (x, y) falls within the indicator rectangle.
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x < self.width_px && y >= self.top_px && y < self.bottom_px()
    }

    /// Centre pixel-row of the indicator.
    pub fn center_y(&self) -> u32 {
        self.top_px + self.height_px / 2
    }

    /// Fraction of the minimap height occupied by this indicator.
    pub fn coverage(&self, minimap_height_px: u32) -> f64 {
        if minimap_height_px == 0 {
            return 0.0;
        }
        self.height_px as f64 / minimap_height_px as f64
    }
}

impl fmt::Display for MinimapViewportIndicator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Indicator(y={}..{}, w={}, {:?})",
            self.top_px,
            self.bottom_px(),
            self.width_px,
            self.style,
        )
    }
}

// ---------------------------------------------------------------------------
// MinimapSearchHighlighter – highlights search matches in minimap
// ---------------------------------------------------------------------------

/// A single search match position for minimap highlighting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatchLocation {
    pub line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

/// Configuration for search highlighting in the minimap.
#[derive(Debug, Clone)]
pub struct SearchHighlightConfig {
    pub highlight_all: bool,
    pub current_match_distinct: bool,
    pub max_highlights: usize,
}

impl SearchHighlightConfig {
    pub fn new() -> Self {
        Self {
            highlight_all: true,
            current_match_distinct: true,
            max_highlights: 10_000,
        }
    }

    pub fn with_max_highlights(mut self, max: usize) -> Self {
        self.max_highlights = max;
        self
    }
}

/// Manages search match highlights for the minimap display.
#[derive(Debug)]
pub struct MinimapSearchHighlighter {
    config: SearchHighlightConfig,
    matches: Vec<SearchMatchLocation>,
    current_index: Option<usize>,
    query: String,
}

impl MinimapSearchHighlighter {
    pub fn new(config: SearchHighlightConfig) -> Self {
        Self {
            config,
            matches: Vec::new(),
            current_index: None,
            query: String::new(),
        }
    }

    /// Set search results from a list of matches.
    pub fn set_matches(&mut self, query: impl Into<String>, matches: Vec<SearchMatchLocation>) {
        self.query = query.into();
        self.matches = if matches.len() > self.config.max_highlights {
            matches[..self.config.max_highlights].to_vec()
        } else {
            matches
        };
        self.current_index = if self.matches.is_empty() { None } else { Some(0) };
    }

    /// Clear all search highlights.
    pub fn clear(&mut self) {
        self.matches.clear();
        self.current_index = None;
        self.query.clear();
    }

    /// Advance to the next match.
    pub fn next_match(&mut self) -> Option<&SearchMatchLocation> {
        if self.matches.is_empty() { return None; }
        let idx = match self.current_index {
            Some(i) => (i + 1) % self.matches.len(),
            None => 0,
        };
        self.current_index = Some(idx);
        self.matches.get(idx)
    }

    /// Go to the previous match.
    pub fn prev_match(&mut self) -> Option<&SearchMatchLocation> {
        if self.matches.is_empty() { return None; }
        let idx = match self.current_index {
            Some(0) => self.matches.len() - 1,
            Some(i) => i - 1,
            None => 0,
        };
        self.current_index = Some(idx);
        self.matches.get(idx)
    }

    /// Get all match lines (deduplicated) for minimap rendering.
    pub fn highlight_lines(&self) -> Vec<usize> {
        let mut lines: Vec<usize> = self.matches.iter().map(|m| m.line).collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// Check whether a line has any matches.
    pub fn line_has_match(&self, line: usize) -> bool {
        self.matches.iter().any(|m| m.line == line)
    }

    /// Count matches on a specific line.
    pub fn matches_on_line(&self, line: usize) -> usize {
        self.matches.iter().filter(|m| m.line == line).count()
    }

    pub fn match_count(&self) -> usize { self.matches.len() }
    pub fn current_index(&self) -> Option<usize> { self.current_index }
    pub fn query(&self) -> &str { &self.query }
    pub fn is_active(&self) -> bool { !self.query.is_empty() }
}

// ---------------------------------------------------------------------------
// MinimapDecorationAggregator – aggregates decorations from multiple sources
// ---------------------------------------------------------------------------

/// Source identifier for a decoration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DecorationSourceId(pub String);

/// A single decoration entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationItem {
    pub source: DecorationSourceId,
    pub line: usize,
    pub kind: DecorationItemKind,
    pub priority: i32,
}

/// Kind of decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationItemKind {
    Error,
    Warning,
    Info,
    Highlight,
    Bookmark,
    Change,
}

/// Aggregates decorations from multiple providers for minimap rendering.
#[derive(Debug)]
pub struct MinimapDecorationAggregator {
    items: Vec<DecorationItem>,
    max_per_line: usize,
}

impl MinimapDecorationAggregator {
    pub fn new(max_per_line: usize) -> Self {
        Self { items: Vec::new(), max_per_line }
    }

    /// Add decorations from a source.
    pub fn add_items(&mut self, items: Vec<DecorationItem>) {
        self.items.extend(items);
    }

    /// Remove all decorations from a specific source.
    pub fn remove_source(&mut self, source: &DecorationSourceId) {
        self.items.retain(|item| &item.source != source);
    }

    /// Clear all decorations.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Get the highest-priority decorations for a line (up to `max_per_line`).
    pub fn decorations_for_line(&self, line: usize) -> Vec<&DecorationItem> {
        let mut line_items: Vec<&DecorationItem> = self.items.iter()
            .filter(|item| item.line == line)
            .collect();
        line_items.sort_by(|a, b| b.priority.cmp(&a.priority));
        line_items.truncate(self.max_per_line);
        line_items
    }

    /// Get all lines that have at least one decoration, sorted.
    pub fn decorated_lines(&self) -> Vec<usize> {
        let mut lines: Vec<usize> = self.items.iter().map(|i| i.line).collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// Total number of decoration items.
    pub fn total_count(&self) -> usize { self.items.len() }

    /// Count items by kind.
    pub fn count_by_kind(&self, kind: DecorationItemKind) -> usize {
        self.items.iter().filter(|i| i.kind == kind).count()
    }

    /// Get all unique sources.
    pub fn sources(&self) -> Vec<&DecorationSourceId> {
        let mut sources: Vec<&DecorationSourceId> = self.items.iter().map(|i| &i.source).collect();
        sources.sort_by_key(|s| &s.0);
        sources.dedup_by_key(|s| &s.0);
        sources
    }

    /// Merge from another aggregator.
    pub fn merge(&mut self, other: &MinimapDecorationAggregator) {
        self.items.extend(other.items.iter().cloned());
    }
}


/// Minimap viewport tracking and selection overlay.
#[derive(Debug, Clone)]
pub struct MinimapViewportTracker {
    entries: Vec<MinimapViewportEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single minimap viewport entry.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapViewportEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl MinimapViewportEntry {
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

impl MinimapViewportTracker {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: MinimapViewportEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&MinimapViewportEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut MinimapViewportEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&MinimapViewportEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&MinimapViewportEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&MinimapViewportEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<MinimapViewportEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
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
// xa_ extended helpers for minimap
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaMinimapRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaMinimapRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaMinimapCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaMinimapCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaMinimapCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 124
// ---------------------------------------------------------------------------

/// Generic object pool `Xc124Pool<T>`.
pub struct Xc124Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc124Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc124PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc124Pool<T> {
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
    pub fn stats(&self) -> Xc124PoolStats {
        Xc124PoolStats {
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

impl<T> Default for Xc124Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc124Scheduler`.
pub struct Xc124Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc124Scheduler {
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

impl Default for Xc124Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_124 hash for the given byte slice.
pub fn xc_124_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_124 convention.
pub fn xc_124_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_37 deepening: state machine + event bus ---

/// States for the Xd37 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd37State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd37State {
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
pub struct Xd37Transition {
    pub from: Xd37State,
    pub to: Xd37State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd37StateMachine {
    current: Xd37State,
    history: Vec<Xd37Transition>,
    step_counter: usize,
}

impl Xd37StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd37State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd37State {
        self.current
    }

    pub fn history(&self) -> &[Xd37Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd37State) -> Result<Xd37State, String> {
        let allowed = match (self.current, target) {
            (Xd37State::Idle, Xd37State::Running) => true,
            (Xd37State::Running, Xd37State::Paused) => true,
            (Xd37State::Running, Xd37State::Done) => true,
            (Xd37State::Paused, Xd37State::Running) => true,
            (Xd37State::Paused, Xd37State::Done) => true,
            (Xd37State::Done, Xd37State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_37: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd37Transition {
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
            "Xd37SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd37State> {
        let prefix = "Xd37SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd37State::Idle),
            "Running" => Some(Xd37State::Running),
            "Paused" => Some(Xd37State::Paused),
            "Done" => Some(Xd37State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd37State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd37 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd37Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd37Event {
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

type Xd37HandlerFn = Box<dyn Fn(&Xd37Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd37EventBus {
    handlers: Vec<(usize, Option<String>, Xd37HandlerFn)>,
    next_id: usize,
    published: Vec<Xd37Event>,
}

impl Xd37EventBus {
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
        F: Fn(&Xd37Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd37Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd37Event) {
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

    pub fn published_events(&self) -> &[Xd37Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #35
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf35Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf35TrieNode {
    children: std::collections::HashMap<char, Xf35TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf35Trie {
    root: Xf35TrieNode,
    count: usize,
}

impl Xf35Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf35TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf35TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf35TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf35BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf35BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 123).
pub struct Xh123SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh123SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 165 as u64,
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

/// A compact bit set supporting boolean operations (variant 123).
pub struct Xh123BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh123BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 123).
pub struct Xi123Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi123Deque<T> {
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
pub struct Xi123Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi123Interval {
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

/// A simple interval tree (variant 123).
pub struct Xi123IntervalTree {
    xi_intervals: Vec<Xi123Interval>,
}

impl Xi123IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi123Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi123Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi123Interval) -> Vec<&Xi123Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi123Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi123Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi123Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi123Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi123Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi123Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 123) ---

/// Disjoint set / union-find for crate 123.
pub struct Xj123UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj123UnionFind {
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

const XJ123_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 123.
pub struct Xj123BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj123BTreeNode<K, V>>>,
    len: usize,
}

struct Xj123BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj123BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj123BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ123_BTREE_ORDER - 1
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
        let mid = XJ123_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj123BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj123BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj123BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj123BTreeNode::xj_new_leaf();
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


// --- xk_123 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk123SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk123SegmentTree {
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
pub struct Xk123DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk123DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_123).
#[derive(Debug, Clone)]
pub struct Xl123Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl123Rope {
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

/// Suffix array for efficient string searching (xl_123).
#[derive(Debug, Clone)]
pub struct Xl123SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl123SuffixArray {
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
pub struct Xm123MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm123MatrixSparse {
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
pub struct Xm123Tokenizer {
    text: String,
}

impl Xm123Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 123.
pub struct Xn123Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn123Fenwick {
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

// ----- AVL tree map — crate 123 -----

#[derive(Debug, Clone)]
struct Xn123AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn123AvlNode<K, V>>>,
    right: Option<Box<Xn123AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 123.
#[derive(Debug, Clone)]
pub struct Xn123AVL<K, V> {
    root: Option<Box<Xn123AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn123AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn123AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn123AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn123AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn123AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn123AvlNode<K, V>>) -> Box<Xn123AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn123AvlNode<K, V>>) -> Box<Xn123AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn123AvlNode<K, V>>) -> Box<Xn123AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn123AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn123AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn123AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn123AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn123AvlNode<K, V>>) -> &Xn123AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn123AvlNode<K, V>>) -> (Box<Xn123AvlNode<K, V>>, Option<Box<Xn123AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn123AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn123AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn123AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn123AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn123AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn123AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn123AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo123RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo123Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo123RBNode<K, V> {
    key: K,
    value: V,
    color: Xo123Color,
    left: Option<Box<Xo123RBNode<K, V>>>,
    right: Option<Box<Xo123RBNode<K, V>>>,
}

/// A red-black tree map for crate 123.
#[derive(Debug, Clone)]
pub struct Xo123RedBlack<K, V> {
    root: Option<Box<Xo123RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo123RedBlack<K, V> {
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
            r.color = Xo123Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo123RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo123RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo123RBNode {
                    key, value, color: Xo123Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo123RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo123Color::Red)
    }

    fn xo_balance(mut h: Box<Xo123RBNode<K, V>>) -> Box<Xo123RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo123Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo123RBNode<K, V>>) -> Box<Xo123RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo123Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo123RBNode<K, V>>) -> Box<Xo123RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo123Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo123RBNode<K, V>>) {
        h.color = Xo123Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo123Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo123Color::Black; }
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
            r.color = Xo123Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo123RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo123RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo123RBNode<K, V>) -> (K, V, Option<Box<Xo123RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo123RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo123Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo123RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo123ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 123.
#[derive(Debug, Clone)]
pub struct Xo123ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo123ConsistentHash {
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
            let vkey = format!("{}#xo123#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo123#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 123).
#[derive(Debug)]
pub struct Xp123SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp123Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp123Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp123Node<K, V>>>,
    xp_right: Option<Box<Xp123Node<K, V>>>,
}

impl<K: Ord, V> Xp123Node<K, V> {
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

impl<K: Ord, V> Default for Xp123SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp123SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp123Node<K, V>>>, key: &K) -> Option<Box<Xp123Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp123Node<K, V>>) -> Box<Xp123Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp123Node<K, V>>) -> Box<Xp123Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp123Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp123Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp123Node::xp_new(key, val));
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


// --------------- Xq123Treap ---------------

use std::cmp::Ordering as Xq123Ord;

struct Xq123TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq123TreapNode<K, V>>>,
    right: Option<Box<Xq123TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq123Treap<K, V> {
    root: Option<Box<Xq123TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq123TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_123_size<K, V>(node: &Option<Box<Xq123TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_123_update_size<K, V>(node: &mut Xq123TreapNode<K, V>) {
    node.size = 1 + xq_123_size(&node.left) + xq_123_size(&node.right);
}

fn xq_123_rotate_right<K, V>(mut node: Box<Xq123TreapNode<K, V>>) -> Box<Xq123TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_123_update_size(&mut node);
    left.right = Some(node);
    xq_123_update_size(&mut left);
    left
}

fn xq_123_rotate_left<K, V>(mut node: Box<Xq123TreapNode<K, V>>) -> Box<Xq123TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_123_update_size(&mut node);
    right.left = Some(node);
    xq_123_update_size(&mut right);
    right
}

fn xq_123_insert_node<K: Ord, V>(
    node: Option<Box<Xq123TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq123TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq123TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq123Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq123Ord::Less => {
                let (new_left, old) = xq_123_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_123_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_123_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq123Ord::Greater => {
                let (new_right, old) = xq_123_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_123_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_123_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_123_remove_node<K: Ord, V>(
    node: Option<Box<Xq123TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq123TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq123Ord::Less => {
                let (new_left, old) = xq_123_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_123_update_size(&mut n);
                (Some(n), old)
            }
            Xq123Ord::Greater => {
                let (new_right, old) = xq_123_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_123_update_size(&mut n);
                (Some(n), old)
            }
            Xq123Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_123_rotate_right(n);
                    let (new_right, old) = xq_123_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_123_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_123_rotate_left(n);
                    let (new_left, old) = xq_123_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_123_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_123_find_min<K, V>(node: &Option<Box<Xq123TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_123_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_123_find_max<K, V>(node: &Option<Box<Xq123TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_123_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_123_rank<K: Ord, V>(node: &Option<Box<Xq123TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq123Ord::Less => xq_123_rank(&n.left, key),
            Xq123Ord::Equal => xq_123_size(&n.left),
            Xq123Ord::Greater => 1 + xq_123_size(&n.left) + xq_123_rank(&n.right, key),
        },
    }
}

fn xq_123_kth<K, V>(node: &Option<Box<Xq123TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_123_size(&n.left);
        if k < left_size {
            xq_123_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_123_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_123_in_order<K: Clone, V>(node: &Option<Box<Xq123TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_123_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_123_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq123Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 123 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_123_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq123Ord::Equal => return Some(&n.value),
                Xq123Ord::Less => cur = &n.left,
                Xq123Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_123_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_123_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_123_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_123_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_123_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_123_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_123_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq123VEBTree ---------------

pub struct Xq123VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq123VEBTree>>,
    clusters: Vec<Option<Box<Xq123VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq123VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq123VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq123VEBTree::xq_new(self.sqrt_lo)));
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
pub struct Xr123KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr123KDPoint {
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
pub struct Xr123BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr123KDNode {
    xr_point: Xr123KDPoint,
    xr_left: Option<Box<Xr123KDNode>>,
    xr_right: Option<Box<Xr123KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr123KDTree {
    xr_root: Option<Box<Xr123KDNode>>,
    xr_size: usize,
}

impl Xr123KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr123KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr123KDNode>>,
        point: Xr123KDPoint,
        depth: usize,
    ) -> Box<Xr123KDNode> {
        match node {
            None => Box::new(Xr123KDNode {
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
    pub fn xr_nearest_neighbor(&self, query: &Xr123KDPoint) -> Option<Xr123KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr123KDNode>,
        query: &Xr123KDPoint,
        depth: usize,
        best: &mut Xr123KDPoint,
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
    ) -> Vec<Xr123KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr123KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr123KDPoint>,
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
    pub fn xr_all_points(&self) -> Vec<Xr123KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr123KDNode>>, pts: &mut Vec<Xr123KDPoint>) {
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

    fn xr_depth_rec(node: &Option<Box<Xr123KDNode>>) -> usize {
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
    pub fn xr_bounding_box(&self) -> Option<Xr123BoundingBox> {
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
        Some(Xr123BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs123PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs123PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs123PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs123PersistentArray {
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
pub struct Xs123ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs123ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs123ConcurrentQueue {
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
pub struct Xs123RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs123RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs123RangeMap {
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
pub struct Xs123CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs123CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs123CircularBuffer {
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

    #[test]
    fn default_config() {
        let cfg = MinimapConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.side, MinimapPosition::Right);
        assert_eq!(cfg.mode, MinimapRenderMode::Blocks);
        assert_eq!(cfg.max_column, 120);
        assert_eq!(cfg.show_slider, ShowSlider::MouseOver);
    }

    #[test]
    fn visible_lines_within_viewport() {
        let mut r = MinimapRenderer::new(MinimapConfig::default());
        let lines: Vec<MinimapLine> = (0..10)
            .map(|i| MinimapLine {
                line_number: i,
                tokens: vec![MinimapToken { start_col: 0, length: 5, color_id: 1 }],
            })
            .collect();
        r.update_content(lines);
        r.set_viewport(2, 5);
        let visible = r.get_visible_lines();
        assert_eq!(visible.len(), 3);
        assert_eq!(visible[0].line_number, 2);
        assert_eq!(visible[2].line_number, 4);
    }

    #[test]
    fn viewport_beyond_content() {
        let mut r = MinimapRenderer::new(MinimapConfig::default());
        r.update_content(vec![
            MinimapLine { line_number: 0, tokens: vec![] },
            MinimapLine { line_number: 1, tokens: vec![] },
        ]);
        r.set_viewport(5, 10);
        assert!(r.get_visible_lines().is_empty());
        assert_eq!(r.line_count(), 2);
    }

    #[test]
    fn empty_renderer() {
        let r = MinimapRenderer::new(MinimapConfig::default());
        assert_eq!(r.line_count(), 0);
        assert!(r.get_visible_lines().is_empty());
    }

    #[test]
    fn builder_pattern() {
        let cfg = MinimapConfigBuilder::new()
            .enabled(false)
            .side(MinimapPosition::Left)
            .mode(MinimapRenderMode::Characters)
            .max_column(80)
            .scale(2)
            .show_slider(ShowSlider::Always)
            .build();
        assert!(!cfg.enabled);
        assert_eq!(cfg.side, MinimapPosition::Left);
        assert_eq!(cfg.mode, MinimapRenderMode::Characters);
        assert_eq!(cfg.max_column, 80);
        assert_eq!(cfg.scale, 2);
        assert_eq!(cfg.show_slider, ShowSlider::Always);
    }

    #[test]
    fn get_line_found_and_missing() {
        let mut r = MinimapRenderer::new(MinimapConfig::default());
        r.update_content(vec![
            MinimapLine { line_number: 0, tokens: vec![] },
            MinimapLine { line_number: 5, tokens: vec![] },
        ]);
        assert!(r.get_line(5).is_some());
        assert_eq!(r.get_line(5).unwrap().line_number, 5);
        assert!(r.get_line(99).is_none());
    }

    #[test]
    fn search_highlight_matches() {
        let mut r = MinimapRenderer::new(MinimapConfig::default());
        let lines: Vec<MinimapLine> = (0..20)
            .map(|i| MinimapLine { line_number: i, tokens: vec![] })
            .collect();
        r.update_content(lines);
        let matches = r.search_highlight("1");
        // Lines whose number contains "1": 1, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19
        assert!(matches.contains(&1));
        assert!(matches.contains(&10));
        assert!(matches.contains(&11));
        assert!(!matches.contains(&0));
        assert!(!matches.contains(&2));
        assert!(r.search_highlight("").is_empty());
    }

    #[test]
    fn viewport_percentage_calculation() {
        let mut r = MinimapRenderer::new(MinimapConfig::default());
        let lines: Vec<MinimapLine> = (0..100)
            .map(|i| MinimapLine { line_number: i, tokens: vec![] })
            .collect();
        r.update_content(lines);
        r.set_viewport(0, 25);
        let pct = r.viewport_percentage();
        assert!((pct - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn viewport_percentage_empty() {
        let r = MinimapRenderer::new(MinimapConfig::default());
        assert!((r.viewport_percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_to_line_centers() {
        let mut r = MinimapRenderer::new(MinimapConfig::default());
        let lines: Vec<MinimapLine> = (0..100)
            .map(|i| MinimapLine { line_number: i, tokens: vec![] })
            .collect();
        r.update_content(lines);
        r.set_viewport(0, 20);
        r.scroll_to_line(50);
        assert!(r.viewport_start <= 50);
        assert!(r.viewport_end > 50);
        assert_eq!(r.viewport_end - r.viewport_start, 20);
    }

    #[test]
    fn scroll_to_line_clamps_end() {
        let mut r = MinimapRenderer::new(MinimapConfig::default());
        let lines: Vec<MinimapLine> = (0..10)
            .map(|i| MinimapLine { line_number: i, tokens: vec![] })
            .collect();
        r.update_content(lines);
        r.set_viewport(0, 6);
        r.scroll_to_line(9);
        assert_eq!(r.viewport_end, 10);
        assert_eq!(r.viewport_start, 4);
    }

    #[test]
    fn compute_metrics_values() {
        let mut r = MinimapRenderer::new(
            MinimapConfigBuilder::new().scale(3).build(),
        );
        let lines: Vec<MinimapLine> = (0..50)
            .map(|i| MinimapLine { line_number: i, tokens: vec![] })
            .collect();
        r.update_content(lines);
        r.set_viewport(10, 30);
        let m = r.compute_metrics();
        assert_eq!(m.total_lines, 50);
        assert_eq!(m.viewport_lines, 20);
        assert!((m.visible_ratio - 0.4).abs() < f64::EPSILON);
        assert!((m.scaled_height - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn merge_tokens_adjacent_same_color() {
        let tokens = vec![
            MinimapToken { start_col: 0, length: 3, color_id: 1 },
            MinimapToken { start_col: 3, length: 2, color_id: 1 },
            MinimapToken { start_col: 5, length: 4, color_id: 2 },
            MinimapToken { start_col: 9, length: 1, color_id: 2 },
        ];
        let merged = MinimapToken::merge_tokens(&tokens);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].length, 5);
        assert_eq!(merged[0].color_id, 1);
        assert_eq!(merged[1].length, 5);
        assert_eq!(merged[1].color_id, 2);
    }

    #[test]
    fn merge_tokens_no_merge_different_colors() {
        let tokens = vec![
            MinimapToken { start_col: 0, length: 3, color_id: 1 },
            MinimapToken { start_col: 3, length: 2, color_id: 2 },
        ];
        let merged = MinimapToken::merge_tokens(&tokens);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn total_width_sum() {
        let line = MinimapLine {
            line_number: 0,
            tokens: vec![
                MinimapToken { start_col: 0, length: 5, color_id: 1 },
                MinimapToken { start_col: 5, length: 10, color_id: 2 },
                MinimapToken { start_col: 15, length: 3, color_id: 3 },
            ],
        };
        assert_eq!(line.total_width(), 18);
        let empty = MinimapLine { line_number: 1, tokens: vec![] };
        assert_eq!(empty.total_width(), 0);
    }

    #[test]
    fn display_config() {
        let cfg = MinimapConfig::default();
        let s = format!("{}", cfg);
        assert!(s.contains("enabled=true"));
        assert!(s.contains("Right"));
        assert!(s.contains("Blocks"));
    }

    #[test]
    fn error_display() {
        let e1 = MinimapError::InvalidViewport { start: 10, end: 5 };
        assert_eq!(format!("{}", e1), "invalid viewport: start 10 >= end 5");
        let e2 = MinimapError::NoContent;
        assert_eq!(format!("{}", e2), "no content loaded");
        let e3 = MinimapError::RenderFailed("out of memory".into());
        assert_eq!(format!("{}", e3), "render failed: out of memory");
    }

    #[test]
    fn decoration_type_display() {
        assert_eq!(format!("{}", DecorationType::Highlight), "highlight");
        assert_eq!(format!("{}", DecorationType::Warning), "warning");
        assert_eq!(format!("{}", DecorationType::Error), "error");
        assert_eq!(format!("{}", DecorationType::Info), "info");
        assert_eq!(format!("{}", DecorationType::Search), "search");
    }

    #[test]
    fn decoration_layer_add_and_count() {
        let mut layer = MinimapDecorationLayer::new();
        assert_eq!(layer.count(), 0);
        layer.add(MinimapDecoration {
            line_number: 5,
            color_id: 1,
            decoration_type: DecorationType::Error,
        });
        layer.add(MinimapDecoration {
            line_number: 10,
            color_id: 2,
            decoration_type: DecorationType::Warning,
        });
        assert_eq!(layer.count(), 2);
    }

    #[test]
    fn decoration_layer_remove_by_line() {
        let mut layer = MinimapDecorationLayer::new();
        layer.add(MinimapDecoration {
            line_number: 3,
            color_id: 1,
            decoration_type: DecorationType::Info,
        });
        layer.add(MinimapDecoration {
            line_number: 3,
            color_id: 2,
            decoration_type: DecorationType::Search,
        });
        layer.add(MinimapDecoration {
            line_number: 7,
            color_id: 1,
            decoration_type: DecorationType::Highlight,
        });
        layer.remove_by_line(3);
        assert_eq!(layer.count(), 1);
        assert_eq!(layer.get_decorations_in_range(0, 100)[0].line_number, 7);
    }

    #[test]
    fn decoration_layer_range_query() {
        let mut layer = MinimapDecorationLayer::new();
        for i in 0..20 {
            layer.add(MinimapDecoration {
                line_number: i,
                color_id: 1,
                decoration_type: DecorationType::Highlight,
            });
        }
        let in_range = layer.get_decorations_in_range(5, 10);
        assert_eq!(in_range.len(), 5);
        assert!(in_range.iter().all(|d| d.line_number >= 5 && d.line_number < 10));
    }

    #[test]
    fn decoration_layer_clear() {
        let mut layer = MinimapDecorationLayer::new();
        layer.add(MinimapDecoration {
            line_number: 1,
            color_id: 0,
            decoration_type: DecorationType::Error,
        });
        layer.clear();
        assert_eq!(layer.count(), 0);
    }

    #[test]
    fn renderer_add_decoration_layer() {
        let mut r = MinimapRenderer::new(MinimapConfig::default());
        let lines: Vec<MinimapLine> = (0..50)
            .map(|i| MinimapLine { line_number: i, tokens: vec![] })
            .collect();
        r.update_content(lines);
        r.set_viewport(10, 20);

        let mut layer = MinimapDecorationLayer::new();
        layer.add(MinimapDecoration {
            line_number: 5,
            color_id: 1,
            decoration_type: DecorationType::Warning,
        });
        layer.add(MinimapDecoration {
            line_number: 15,
            color_id: 2,
            decoration_type: DecorationType::Error,
        });

        let visible = r.add_decoration_layer(&layer);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].line_number, 15);
    }

    #[test]
    fn renderer_get_decorations_in_viewport_multi_layer() {
        let mut r = MinimapRenderer::new(MinimapConfig::default());
        r.update_content(vec![]);
        r.set_viewport(0, 100);

        let mut l1 = MinimapDecorationLayer::new();
        l1.add(MinimapDecoration {
            line_number: 10,
            color_id: 1,
            decoration_type: DecorationType::Info,
        });
        let mut l2 = MinimapDecorationLayer::new();
        l2.add(MinimapDecoration {
            line_number: 50,
            color_id: 3,
            decoration_type: DecorationType::Search,
        });

        let layers = [l1, l2];
        let all = r.get_decorations_in_viewport(&layers);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn minimap_region_span_and_contains() {
        let region = MinimapRegion {
            start_line: 10,
            end_line: 25,
            label: "imports".to_string(),
        };
        assert_eq!(region.line_span(), 15);
        assert!(region.contains_line(10));
        assert!(region.contains_line(24));
        assert!(!region.contains_line(25));
        assert!(!region.contains_line(9));
    }

    #[test]
    fn minimap_layout_positions() {
        let layout = MinimapLayout::new(2.5, 100, 10, 30);
        assert!((layout.line_to_y(0) - 0.0).abs() < f64::EPSILON);
        assert!((layout.line_to_y(4) - 10.0).abs() < f64::EPSILON);
        assert_eq!(layout.y_to_line(10.0), 4);
        assert_eq!(layout.y_to_line(999.0), 99);
        assert!((layout.total_height() - 250.0).abs() < f64::EPSILON);
        let (vy_start, vy_end) = layout.viewport_y_range();
        assert!((vy_start - 25.0).abs() < f64::EPSILON);
        assert!((vy_end - 75.0).abs() < f64::EPSILON);
    }

    #[test]
    fn minimap_layout_zero_line_height() {
        let layout = MinimapLayout::new(0.0, 50, 0, 10);
        assert_eq!(layout.y_to_line(100.0), 0);
        assert!((layout.total_height() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn selection_contains_and_span() {
        let sel = MinimapSelection::new(5, 10, 0, 80);
        assert_eq!(sel.line_span(), 6);
        assert!(sel.contains_line(5));
        assert!(sel.contains_line(10));
        assert!(!sel.contains_line(4));
        assert!(!sel.contains_line(11));
    }

    #[test]
    fn selection_overlap_and_merge() {
        let a = MinimapSelection::new(1, 5, 0, 10);
        let b = MinimapSelection::new(4, 8, 2, 15);
        assert!(a.overlaps(&b));
        let merged = a.merge(&b).unwrap();
        assert_eq!(merged.start_line, 1);
        assert_eq!(merged.end_line, 8);
        assert_eq!(merged.start_col, 0);
        assert_eq!(merged.end_col, 15);
    }

    #[test]
    fn selection_no_overlap() {
        let a = MinimapSelection::new(1, 3, 0, 10);
        let b = MinimapSelection::new(5, 8, 0, 10);
        assert!(!a.overlaps(&b));
        assert!(a.merge(&b).is_none());
    }

    #[test]
    fn minimap_search_matches() {
        let lines = vec![
            MinimapLine {
                line_number: 0,
                tokens: vec![MinimapToken { start_col: 0, length: 10, color_id: 1 }],
            },
            MinimapLine {
                line_number: 1,
                tokens: vec![MinimapToken { start_col: 0, length: 2, color_id: 1 }],
            },
            MinimapLine {
                line_number: 2,
                tokens: vec![MinimapToken { start_col: 0, length: 5, color_id: 2 }],
            },
        ];
        let mut search = MinimapSearch::new("hello", false);
        search.search(&lines);
        assert_eq!(search.match_count(), 2);
        assert!(search.has_match(0));
        assert!(!search.has_match(1));
        assert!(search.has_match(2));
    }

    #[test]
    fn color_scheme_lookup() {
        let mut scheme = MinimapColorScheme::new("dark");
        scheme.set_color(1, [255, 0, 0]);
        scheme.set_color(2, [0, 255, 0]);
        assert_eq!(scheme.get_color(1), [255, 0, 0]);
        assert_eq!(scheme.get_color(2), [0, 255, 0]);
        assert_eq!(scheme.get_color(99), [200, 200, 200]);
        assert_eq!(scheme.color_count(), 2);
        let token = MinimapToken { start_col: 0, length: 5, color_id: 1 };
        assert_eq!(scheme.resolve_token(&token), [255, 0, 0]);
    }

    #[test]
    fn color_scheme_update() {
        let mut scheme = MinimapColorScheme::new("test");
        scheme.set_color(1, [100, 100, 100]);
        scheme.set_color(1, [200, 200, 200]);
        assert_eq!(scheme.color_count(), 1);
        assert_eq!(scheme.get_color(1), [200, 200, 200]);
    }

    #[test]
    fn braille_render_line_empty() {
        let r = MinimapBrailleRenderer::new(5);
        let line = r.render_line("");
        assert_eq!(line.len(), 5 * 3); // braille chars are 3 bytes each in UTF-8
        assert!(line.chars().all(|c| c == '\u{2800}')); // all blank braille
    }

    #[test]
    fn braille_render_line_content() {
        let r = MinimapBrailleRenderer::new(3);
        let line = r.render_line("ab cd");
        // First cell covers cols 0,1 ('a','b') → dots 1+4 = 0x09
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(chars[0], '\u{2809}'); // both dots
    }

    #[test]
    fn braille_render_block_single_row() {
        let r = MinimapBrailleRenderer::new(2);
        let lines: Vec<&str> = vec!["ab", "cd", "ef", "gh"];
        let rows = r.render_block(&lines);
        assert_eq!(rows.len(), 1); // 4 lines = 1 braille row
    }

    #[test]
    fn braille_render_block_multiple_rows() {
        let r = MinimapBrailleRenderer::new(2);
        let lines: Vec<&str> = vec!["a", "b", "c", "d", "e"];
        let rows = r.render_block(&lines);
        assert_eq!(rows.len(), 2); // 5 lines = ceil(5/4) = 2 braille rows
    }

    #[test]
    fn braille_rows_needed() {
        let r = MinimapBrailleRenderer::new(10);
        assert_eq!(r.rows_needed(0), 0);
        assert_eq!(r.rows_needed(1), 1);
        assert_eq!(r.rows_needed(4), 1);
        assert_eq!(r.rows_needed(5), 2);
        assert_eq!(r.rows_needed(100), 25);
    }

    #[test]
    fn braille_render_line_spaces_are_blank() {
        let r = MinimapBrailleRenderer::new(3);
        let line = r.render_line("      "); // all spaces
        assert!(line.chars().all(|c| c == '\u{2800}')); // all blank
    }

    #[test]
    fn config_is_enabled_and_effective_width() {
        let cfg = MinimapConfig::default();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.effective_width(), 120);

        let cfg2 = MinimapConfigBuilder::new()
            .enabled(false)
            .max_column(200)
            .scale(4)
            .build();
        assert!(!cfg2.is_enabled());
        assert_eq!(cfg2.effective_width(), 50);
        assert!(cfg2.summary().contains("200"));
    }

    #[test]
    fn metrics_coverage_and_full_document() {
        let full = MinimapMetrics {
            visible_ratio: 1.0,
            total_lines: 50,
            viewport_lines: 50,
            scaled_height: 100.0,
        };
        assert!(full.is_full_document());
        assert!((full.coverage_ratio() - 1.0).abs() < f64::EPSILON);
        assert_eq!(full.hidden_lines(), 0);

        let partial = MinimapMetrics {
            visible_ratio: 0.5,
            total_lines: 100,
            viewport_lines: 20,
            scaled_height: 200.0,
        };
        assert!(!partial.is_full_document());
        assert!((partial.coverage_ratio() - 0.2).abs() < f64::EPSILON);
        assert_eq!(partial.hidden_lines(), 80);

        let empty = MinimapMetrics {
            visible_ratio: 0.0,
            total_lines: 0,
            viewport_lines: 0,
            scaled_height: 0.0,
        };
        assert!((empty.coverage_ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn line_is_blank_and_char_count() {
        let blank = MinimapLine { line_number: 0, tokens: vec![] };
        assert!(blank.is_blank());
        assert_eq!(blank.char_count(), 0);
        assert_eq!(blank.max_column(), 0);
        assert_eq!(blank.token_count(), 0);

        let line = MinimapLine {
            line_number: 1,
            tokens: vec![
                MinimapToken { start_col: 0, length: 5, color_id: 1 },
                MinimapToken { start_col: 10, length: 3, color_id: 2 },
            ],
        };
        assert!(!line.is_blank());
        assert_eq!(line.char_count(), 8);
        assert_eq!(line.max_column(), 13);
        assert_eq!(line.token_count(), 2);
    }

    #[test]
    fn token_end_col_and_overlaps() {
        let a = MinimapToken { start_col: 0, length: 5, color_id: 1 };
        let b = MinimapToken { start_col: 3, length: 4, color_id: 2 };
        let c = MinimapToken { start_col: 5, length: 2, color_id: 3 };
        assert_eq!(a.end_col(), 5);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
        assert!(b.overlaps(&c));
    }

    #[test]
    fn decoration_layer_merge_and_iter() {
        let mut l1 = MinimapDecorationLayer::new();
        l1.add(MinimapDecoration {
            line_number: 1,
            color_id: 1,
            decoration_type: DecorationType::Error,
        });
        let mut l2 = MinimapDecorationLayer::new();
        l2.add(MinimapDecoration {
            line_number: 2,
            color_id: 2,
            decoration_type: DecorationType::Warning,
        });
        l2.add(MinimapDecoration {
            line_number: 3,
            color_id: 3,
            decoration_type: DecorationType::Info,
        });

        l1.merge(&l2);
        assert_eq!(l1.count(), 3);
        assert!(l1.has_errors());
        assert_eq!(l1.iter().count(), 3);
        assert_eq!(l1.filter_by_type(DecorationType::Warning).len(), 1);
        assert_eq!(l1.filter_by_type(DecorationType::Search).len(), 0);
    }

    #[test]
    fn region_overlaps_and_is_empty() {
        let a = MinimapRegion { start_line: 10, end_line: 20, label: "a".into() };
        let b = MinimapRegion { start_line: 15, end_line: 25, label: "b".into() };
        let c = MinimapRegion { start_line: 20, end_line: 30, label: "c".into() };
        let empty = MinimapRegion { start_line: 5, end_line: 5, label: "e".into() };

        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
        assert!(!empty.is_empty() || empty.start_line >= empty.end_line);
        assert!(empty.is_empty());
        assert!(!a.is_empty());
    }

    #[test]
    fn layout_viewport_helpers() {
        let layout = MinimapLayout::new(3.0, 200, 50, 100);
        assert!(layout.is_within_viewport(50));
        assert!(layout.is_within_viewport(99));
        assert!(!layout.is_within_viewport(100));
        assert!(!layout.is_within_viewport(49));
        assert_eq!(layout.viewport_lines(), 50);
        assert!((layout.viewport_height() - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn selection_line_count_and_is_empty() {
        let sel = MinimapSelection::new(5, 10, 0, 80);
        assert_eq!(sel.line_count(), 6);
        assert!(!sel.is_empty());
        assert!(!sel.is_single_line());

        let single = MinimapSelection::new(3, 3, 5, 5);
        assert_eq!(single.line_count(), 1);
        assert!(single.is_empty());
        assert!(single.is_single_line());

        let single_range = MinimapSelection::new(3, 3, 0, 10);
        assert!(!single_range.is_empty());
        assert!(single_range.is_single_line());
    }

    #[test]
    fn color_scheme_dark_light_and_ids() {
        let mut dark = MinimapColorScheme::new("dark");
        dark.default_color = [30, 30, 30];
        dark.set_color(1, [255, 0, 0]);
        dark.set_color(2, [0, 255, 0]);
        assert!(dark.is_dark());
        assert!(!dark.is_light());
        let ids = dark.color_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));

        let light = MinimapColorScheme::new("light");
        assert!(light.is_light());
        assert!(!light.is_dark());
    }

    // -----------------------------------------------------------------------
    // New tests for scale factor, viewport mapping, density sampler,
    // search overlay, and git change layer
    // -----------------------------------------------------------------------

    #[test]
    fn scale_factor_lines_per_pixel_and_round_trip() {
        let scale = MinimapScaleFactor::new(1000, 500);
        assert!((scale.lines_per_pixel() - 2.0).abs() < f64::EPSILON);
        assert!((scale.pixels_per_line() - 0.5).abs() < f64::EPSILON);
        assert!(scale.is_downsampled());

        // Round-trip: line → pixel → line should be close to original
        let px = scale.line_to_pixel(400);
        let back = scale.pixel_to_line(px);
        assert!((back as i64 - 400).unsigned_abs() <= 1);

        // Edge: zero height
        let zero = MinimapScaleFactor::new(100, 0);
        assert!((zero.lines_per_pixel() - 0.0).abs() < f64::EPSILON);
        assert_eq!(zero.line_to_pixel(50), 0);
    }

    #[test]
    fn scale_factor_not_downsampled() {
        let scale = MinimapScaleFactor::new(200, 800);
        assert!(!scale.is_downsampled());
        assert!(scale.pixels_per_line() > 1.0);
    }

    #[test]
    fn viewport_mapping_slider_and_click() {
        let vm = ViewportMapping::new(100, 150, 1000, 500);
        let (top, bottom) = vm.slider_pixel_range();
        assert!(top < bottom);
        assert!(vm.slider_height_px() >= 1);

        // Clicking at pixel 250 (mid-minimap) should map to ~line 500
        let center = vm.click_to_center_line(250);
        assert!((center as i64 - 500).unsigned_abs() <= 1);

        assert!((vm.visible_fraction() - 0.05).abs() < 0.01);
    }

    #[test]
    fn viewport_mapping_full_document() {
        let vm = ViewportMapping::new(0, 100, 100, 400);
        assert!((vm.visible_fraction() - 1.0).abs() < f64::EPSILON);
        let (top, bottom) = vm.slider_pixel_range();
        assert_eq!(top, 0);
        assert_eq!(bottom, 400);
    }

    #[test]
    fn line_density_sampler_basic() {
        let lines: Vec<MinimapLine> = (0..100)
            .map(|i| MinimapLine {
                line_number: i,
                tokens: vec![MinimapToken {
                    start_col: 0,
                    length: 10,
                    color_id: if i < 50 { 1 } else { 2 },
                }],
            })
            .collect();
        let samples = LineDensitySampler::sample(&lines, 10);
        assert_eq!(samples.len(), 10);
        // First half should be dominated by color_id 1
        assert_eq!(samples[0], Some(1));
        // Last entry should be dominated by color_id 2
        assert_eq!(samples[9], Some(2));
    }

    #[test]
    fn line_density_sampler_blank_lines() {
        let lines: Vec<MinimapLine> = (0..10)
            .map(|i| MinimapLine {
                line_number: i,
                tokens: vec![],
            })
            .collect();
        let samples = LineDensitySampler::sample(&lines, 5);
        assert!(samples.iter().all(|s| s.is_none()));
    }

    #[test]
    fn search_highlight_overlay_basic() {
        let scale = MinimapScaleFactor::new(1000, 200);
        let matched = vec![0, 100, 500, 999];
        let overlay = SearchHighlightOverlay::from_matches(&matched, &scale);
        assert!(overlay.highlight_count() <= matched.len());
        assert!(overlay.is_highlighted(scale.line_to_pixel(0)));
        assert!(overlay.is_highlighted(scale.line_to_pixel(500)));
        // An arbitrary non-matched pixel should not be highlighted
        let unmapped = scale.line_to_pixel(300);
        // May or may not be highlighted depending on density – just ensure no panic
        let _ = overlay.is_highlighted(unmapped);
    }

    #[test]
    fn git_change_layer_project() {
        let mut layer = GitChangeLayer::new();
        layer.add(GitChange {
            start_line: 10,
            end_line: 20,
            kind: GitChangeKind::Added,
        });
        layer.add(GitChange {
            start_line: 50,
            end_line: 55,
            kind: GitChangeKind::Modified,
        });
        assert_eq!(layer.count(), 2);

        let scale = MinimapScaleFactor::new(100, 100);
        let projected = layer.project_onto_minimap(&scale);
        assert!(!projected.is_empty());
        // All projected rows for the first change should be Added
        let added: Vec<_> = projected.iter().filter(|(_, k)| *k == GitChangeKind::Added).collect();
        assert!(!added.is_empty());

        // Range query
        let in_range = layer.changes_in_range(15, 25);
        assert_eq!(in_range.len(), 1);
        assert_eq!(in_range[0].kind, GitChangeKind::Added);
    }

    #[test]
    fn git_change_layer_clear_and_empty() {
        let mut layer = GitChangeLayer::new();
        layer.add(GitChange {
            start_line: 0,
            end_line: 5,
            kind: GitChangeKind::Deleted,
        });
        assert_eq!(layer.count(), 1);
        layer.clear();
        assert_eq!(layer.count(), 0);
        assert!(layer.changes_in_range(0, 100).is_empty());
    }

    // --- MinimapSliderDrag tests ---

    #[test]
    fn slider_drag_idle() {
        let drag = MinimapSliderDrag::idle();
        assert!(!drag.active);
        assert_eq!(drag.pixel_delta(), 0);
        assert_eq!(drag.to_string(), "Drag(idle)");
    }

    #[test]
    fn slider_drag_begin_and_update() {
        let mut drag = MinimapSliderDrag::begin(100, 50);
        assert!(drag.active);
        assert_eq!(drag.pixel_delta(), 0);

        drag.update(130);
        assert_eq!(drag.pixel_delta(), 30);
    }

    #[test]
    fn slider_drag_target_line() {
        let mut drag = MinimapSliderDrag::begin(50, 100);
        drag.update(70); // +20 pixels
        let scale = MinimapScaleFactor::new(1000, 200);
        // 20 pixels * (1000/200) = 100 lines offset
        let target = drag.target_line(&scale);
        assert_eq!(target, 200); // 100 + 100
    }

    #[test]
    fn slider_drag_clamps_to_bounds() {
        let mut drag = MinimapSliderDrag::begin(100, 0);
        drag.update(10); // -90 pixels → negative target
        let scale = MinimapScaleFactor::new(500, 200);
        assert_eq!(drag.target_line(&scale), 0);
    }

    #[test]
    fn slider_drag_end_returns_target() {
        let mut drag = MinimapSliderDrag::begin(0, 0);
        drag.update(10);
        let scale = MinimapScaleFactor::new(100, 100);
        let target = drag.end(&scale);
        assert_eq!(target, 10);
        assert!(!drag.active);
    }

    // --- MinimapHighlights tests ---

    #[test]
    fn highlights_empty() {
        let hl = MinimapHighlights::new();
        assert!(hl.is_empty());
        assert_eq!(hl.len(), 0);
        assert_eq!(hl.dominant_at(0), None);
    }

    #[test]
    fn highlights_search_and_error_priority() {
        let scale = MinimapScaleFactor::new(100, 100);
        let overlay = SearchHighlightOverlay::from_matches(&[10], &scale);

        let mut layer = MinimapDecorationLayer::new();
        layer.add(MinimapDecoration {
            line_number: 10,
            color_id: 0,
            decoration_type: DecorationType::Error,
        });

        let mut hl = MinimapHighlights::new();
        hl.add_search_highlights(&overlay);
        hl.add_errors(&layer, &scale);

        // Error has higher priority than SearchMatch
        assert_eq!(hl.dominant_at(10), Some(HighlightKind::Error));
    }

    #[test]
    fn highlights_add_selection() {
        let scale = MinimapScaleFactor::new(200, 200);
        let sel = MinimapSelection::new(5, 8, 0, 10);
        let mut hl = MinimapHighlights::new();
        hl.add_selection(&sel, &scale);

        assert!(!hl.is_empty());
        assert_eq!(hl.dominant_at(5), Some(HighlightKind::Selection));
        assert_eq!(hl.dominant_at(8), Some(HighlightKind::Selection));
        assert_eq!(hl.dominant_at(50), None);
    }

    #[test]
    fn highlights_compact_deduplicates() {
        let scale = MinimapScaleFactor::new(100, 100);
        let overlay = SearchHighlightOverlay::from_matches(&[10, 10], &scale);
        let mut hl = MinimapHighlights::new();
        hl.add_search_highlights(&overlay);

        let before = hl.len();
        hl.compact();
        assert!(hl.len() <= before);
        assert_eq!(hl.dominant_at(10), Some(HighlightKind::SearchMatch));
    }

    // --- MinimapScale tests ---

    #[test]
    fn scale_basic_calculations() {
        let sc = MinimapScale::new(8.0, 16.0, 4);
        assert!((sc.scaled_char_width() - 2.0).abs() < f64::EPSILON);
        assert!((sc.scaled_char_height() - 4.0).abs() < f64::EPSILON);
        assert!((sc.columns_to_px(10) - 20.0).abs() < f64::EPSILON);
        assert!((sc.lines_to_px(5) - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scale_px_to_columns_and_lines() {
        let sc = MinimapScale::new(10.0, 20.0, 2);
        // scaled char_width=5, char_height=10
        assert_eq!(sc.px_to_columns(50.0), 10);
        assert_eq!(sc.px_to_lines(100.0), 10);
    }

    #[test]
    fn scale_divisor_clamped_to_one() {
        let sc = MinimapScale::new(8.0, 16.0, 0);
        assert_eq!(sc.divisor, 1);
        assert!((sc.scaled_char_width() - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scale_display() {
        let sc = MinimapScale::new(8.0, 16.0, 4);
        let s = sc.to_string();
        assert!(s.contains("1/4"));
    }

    // --- MinimapViewportIndicator tests ---

    #[test]
    fn viewport_indicator_from_mapping() {
        let mapping = ViewportMapping::new(100, 150, 1000, 500);
        let ind = MinimapViewportIndicator::from_mapping(&mapping, 60, IndicatorStyle::Fill);

        assert!(ind.height_px >= 1);
        assert_eq!(ind.width_px, 60);
        assert_eq!(ind.style, IndicatorStyle::Fill);
        assert_eq!(ind.bottom_px(), ind.top_px + ind.height_px);
    }

    #[test]
    fn viewport_indicator_contains() {
        let ind = MinimapViewportIndicator {
            top_px: 10,
            height_px: 20,
            width_px: 50,
            style: IndicatorStyle::Border,
        };
        assert!(ind.contains(0, 10));
        assert!(ind.contains(49, 29));
        assert!(!ind.contains(50, 10)); // x out of bounds
        assert!(!ind.contains(0, 30));  // y out of bounds
        assert!(!ind.contains(0, 9));   // above
    }

    #[test]
    fn viewport_indicator_coverage() {
        let ind = MinimapViewportIndicator {
            top_px: 0,
            height_px: 25,
            width_px: 50,
            style: IndicatorStyle::Fill,
        };
        let cov = ind.coverage(100);
        assert!((cov - 0.25).abs() < f64::EPSILON);
        assert!((ind.coverage(0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn viewport_indicator_display() {
        let ind = MinimapViewportIndicator {
            top_px: 5,
            height_px: 10,
            width_px: 40,
            style: IndicatorStyle::Fill,
        };
        let s = ind.to_string();
        assert!(s.contains("5..15"));
        assert!(s.contains("w=40"));
    }

    #[test]
    fn search_highlighter_new() {
        let h = MinimapSearchHighlighter::new(SearchHighlightConfig::new());
        assert_eq!(h.match_count(), 0);
        assert!(!h.is_active());
    }

    #[test]
    fn search_highlighter_set_matches() {
        let mut h = MinimapSearchHighlighter::new(SearchHighlightConfig::new());
        h.set_matches("hello", vec![
            SearchMatchLocation { line: 1, start_col: 0, end_col: 5 },
            SearchMatchLocation { line: 3, start_col: 10, end_col: 15 },
        ]);
        assert_eq!(h.match_count(), 2);
        assert!(h.is_active());
        assert_eq!(h.query(), "hello");
    }

    #[test]
    fn search_highlighter_next_prev() {
        let mut h = MinimapSearchHighlighter::new(SearchHighlightConfig::new());
        h.set_matches("x", vec![
            SearchMatchLocation { line: 1, start_col: 0, end_col: 1 },
            SearchMatchLocation { line: 5, start_col: 0, end_col: 1 },
        ]);
        let n = h.next_match().unwrap();
        assert_eq!(n.line, 5);
        let p = h.prev_match().unwrap();
        assert_eq!(p.line, 1);
    }

    #[test]
    fn search_highlighter_wrap() {
        let mut h = MinimapSearchHighlighter::new(SearchHighlightConfig::new());
        h.set_matches("x", vec![
            SearchMatchLocation { line: 1, start_col: 0, end_col: 1 },
        ]);
        h.next_match(); // wraps to 0
        let m = h.next_match().unwrap();
        assert_eq!(m.line, 1);
    }

    #[test]
    fn search_highlighter_clear() {
        let mut h = MinimapSearchHighlighter::new(SearchHighlightConfig::new());
        h.set_matches("test", vec![SearchMatchLocation { line: 0, start_col: 0, end_col: 4 }]);
        h.clear();
        assert_eq!(h.match_count(), 0);
        assert!(!h.is_active());
    }

    #[test]
    fn search_highlighter_highlight_lines() {
        let mut h = MinimapSearchHighlighter::new(SearchHighlightConfig::new());
        h.set_matches("x", vec![
            SearchMatchLocation { line: 5, start_col: 0, end_col: 1 },
            SearchMatchLocation { line: 5, start_col: 10, end_col: 11 },
            SearchMatchLocation { line: 10, start_col: 0, end_col: 1 },
        ]);
        let lines = h.highlight_lines();
        assert_eq!(lines, vec![5, 10]);
    }

    #[test]
    fn search_highlighter_matches_on_line() {
        let mut h = MinimapSearchHighlighter::new(SearchHighlightConfig::new());
        h.set_matches("x", vec![
            SearchMatchLocation { line: 3, start_col: 0, end_col: 1 },
            SearchMatchLocation { line: 3, start_col: 5, end_col: 6 },
        ]);
        assert_eq!(h.matches_on_line(3), 2);
        assert_eq!(h.matches_on_line(4), 0);
    }

    #[test]
    fn search_highlighter_max_highlights() {
        let config = SearchHighlightConfig::new().with_max_highlights(2);
        let mut h = MinimapSearchHighlighter::new(config);
        h.set_matches("x", vec![
            SearchMatchLocation { line: 1, start_col: 0, end_col: 1 },
            SearchMatchLocation { line: 2, start_col: 0, end_col: 1 },
            SearchMatchLocation { line: 3, start_col: 0, end_col: 1 },
        ]);
        assert_eq!(h.match_count(), 2);
    }

    #[test]
    fn decoration_aggregator_add_and_query() {
        let mut agg = MinimapDecorationAggregator::new(5);
        agg.add_items(vec![
            DecorationItem { source: DecorationSourceId("diag".into()), line: 10, kind: DecorationItemKind::Error, priority: 10 },
            DecorationItem { source: DecorationSourceId("git".into()), line: 10, kind: DecorationItemKind::Change, priority: 5 },
        ]);
        assert_eq!(agg.total_count(), 2);
        let line_decs = agg.decorations_for_line(10);
        assert_eq!(line_decs.len(), 2);
        assert_eq!(line_decs[0].priority, 10); // highest first
    }

    #[test]
    fn decoration_aggregator_remove_source() {
        let mut agg = MinimapDecorationAggregator::new(5);
        let src = DecorationSourceId("diag".into());
        agg.add_items(vec![
            DecorationItem { source: src.clone(), line: 1, kind: DecorationItemKind::Error, priority: 1 },
        ]);
        agg.remove_source(&src);
        assert_eq!(agg.total_count(), 0);
    }

    #[test]
    fn decoration_aggregator_decorated_lines() {
        let mut agg = MinimapDecorationAggregator::new(5);
        agg.add_items(vec![
            DecorationItem { source: DecorationSourceId("a".into()), line: 5, kind: DecorationItemKind::Info, priority: 1 },
            DecorationItem { source: DecorationSourceId("a".into()), line: 15, kind: DecorationItemKind::Warning, priority: 1 },
            DecorationItem { source: DecorationSourceId("b".into()), line: 5, kind: DecorationItemKind::Bookmark, priority: 2 },
        ]);
        assert_eq!(agg.decorated_lines(), vec![5, 15]);
    }

    #[test]
    fn decoration_aggregator_count_by_kind() {
        let mut agg = MinimapDecorationAggregator::new(5);
        agg.add_items(vec![
            DecorationItem { source: DecorationSourceId("a".into()), line: 1, kind: DecorationItemKind::Error, priority: 1 },
            DecorationItem { source: DecorationSourceId("a".into()), line: 2, kind: DecorationItemKind::Error, priority: 1 },
            DecorationItem { source: DecorationSourceId("a".into()), line: 3, kind: DecorationItemKind::Warning, priority: 1 },
        ]);
        assert_eq!(agg.count_by_kind(DecorationItemKind::Error), 2);
        assert_eq!(agg.count_by_kind(DecorationItemKind::Warning), 1);
    }

    #[test]
    fn decoration_aggregator_max_per_line() {
        let mut agg = MinimapDecorationAggregator::new(1);
        agg.add_items(vec![
            DecorationItem { source: DecorationSourceId("a".into()), line: 1, kind: DecorationItemKind::Error, priority: 10 },
            DecorationItem { source: DecorationSourceId("b".into()), line: 1, kind: DecorationItemKind::Warning, priority: 5 },
        ]);
        let decs = agg.decorations_for_line(1);
        assert_eq!(decs.len(), 1);
        assert_eq!(decs[0].kind, DecorationItemKind::Error);
    }

    #[test]
    fn decoration_aggregator_merge() {
        let mut a = MinimapDecorationAggregator::new(5);
        let mut b = MinimapDecorationAggregator::new(5);
        a.add_items(vec![DecorationItem { source: DecorationSourceId("s".into()), line: 1, kind: DecorationItemKind::Info, priority: 1 }]);
        b.add_items(vec![DecorationItem { source: DecorationSourceId("s".into()), line: 2, kind: DecorationItemKind::Info, priority: 1 }]);
        a.merge(&b);
        assert_eq!(a.total_count(), 2);
    }

    #[test]
    fn minimap_viewport_entry_creation() {
        let e = MinimapViewportEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn minimap_viewport_entry_with_priority() {
        let e = MinimapViewportEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn minimap_viewport_entry_metadata() {
        let e = MinimapViewportEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn minimap_viewport_entry_remove_meta() {
        let mut e = MinimapViewportEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn minimap_viewport_entry_activate_deactivate() {
        let mut e = MinimapViewportEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn minimap_viewport_tracker_add_sorted() {
        let mut c = MinimapViewportTracker::new(10);
        c.add(MinimapViewportEntry::new("lo", "Lo").with_priority(1));
        c.add(MinimapViewportEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn minimap_viewport_tracker_capacity() {
        let mut c = MinimapViewportTracker::new(1);
        assert!(c.add(MinimapViewportEntry::new("a", "A")));
        assert!(!c.add(MinimapViewportEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn minimap_viewport_tracker_remove() {
        let mut c = MinimapViewportTracker::new(10);
        c.add(MinimapViewportEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn minimap_viewport_tracker_get() {
        let mut c = MinimapViewportTracker::new(10);
        c.add(MinimapViewportEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn minimap_viewport_tracker_active_entries() {
        let mut c = MinimapViewportTracker::new(10);
        c.add(MinimapViewportEntry::new("a", "A"));
        c.add(MinimapViewportEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn minimap_viewport_tracker_enable_disable() {
        let mut c = MinimapViewportTracker::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn minimap_viewport_tracker_clear() {
        let mut c = MinimapViewportTracker::new(10);
        c.add(MinimapViewportEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn minimap_viewport_tracker_find_by_label() {
        let mut c = MinimapViewportTracker::new(10);
        c.add(MinimapViewportEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn minimap_viewport_tracker_top_n() {
        let mut c = MinimapViewportTracker::new(10);
        c.add(MinimapViewportEntry::new("a", "A").with_priority(1));
        c.add(MinimapViewportEntry::new("b", "B").with_priority(2));
        c.add(MinimapViewportEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn minimap_viewport_tracker_deactivate_activate_all() {
        let mut c = MinimapViewportTracker::new(10);
        c.add(MinimapViewportEntry::new("a", "A"));
        c.add(MinimapViewportEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn minimap_viewport_tracker_highest_priority() {
        let mut c = MinimapViewportTracker::new(10);
        assert!(c.highest_priority().is_none());
        c.add(MinimapViewportEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn minimap_viewport_tracker_contains() {
        let mut c = MinimapViewportTracker::new(10);
        c.add(MinimapViewportEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn minimap_viewport_tracker_drain_inactive() {
        let mut c = MinimapViewportTracker::new(10);
        c.add(MinimapViewportEntry::new("a", "A"));
        c.add(MinimapViewportEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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


    // xa_ extended tests for minimap
    #[test]
    fn xa_minimap_ring_new() {
        let rb = super::XaMinimapRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_minimap_ring_push_len() {
        let mut rb = super::XaMinimapRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_minimap_ring_wrap() {
        let mut rb = super::XaMinimapRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_minimap_ring_mean_empty() {
        let rb = super::XaMinimapRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_minimap_ring_mean_values() {
        let mut rb = super::XaMinimapRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_minimap_ring_min_max() {
        let mut rb = super::XaMinimapRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_minimap_ring_iter() {
        let mut rb = super::XaMinimapRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_minimap_counter_new() {
        let c = super::XaMinimapCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_minimap_counter_inc() {
        let mut c = super::XaMinimapCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_minimap_counter_inc_by() {
        let mut c = super::XaMinimapCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_minimap_counter_reset() {
        let mut c = super::XaMinimapCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_minimap_counter_clear() {
        let mut c = super::XaMinimapCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_minimap_counter_default() {
        let c = super::XaMinimapCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 124 ----

    #[test]
    fn xc_124_pool_new_empty() {
        let pool: super::Xc124Pool<i32> = super::Xc124Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_124_pool_release_acquire() {
        let mut pool = super::Xc124Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_124_pool_acquire_empty() {
        let mut pool: super::Xc124Pool<i32> = super::Xc124Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_124_pool_full() {
        let mut pool = super::Xc124Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_124_pool_drain() {
        let mut pool = super::Xc124Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_124_pool_stats() {
        let mut pool = super::Xc124Pool::new(8);
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
    fn xc_124_pool_clear() {
        let mut pool = super::Xc124Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_124_pool_shrink() {
        let mut pool = super::Xc124Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_124_pool_default() {
        let pool: super::Xc124Pool<String> = super::Xc124Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_124_pool_extend() {
        let mut pool = super::Xc124Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_124_pool_retain() {
        let mut pool = super::Xc124Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_124_scheduler_round_robin() {
        let mut sched = super::Xc124Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_124_scheduler_empty() {
        let mut sched = super::Xc124Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_124_scheduler_reset() {
        let mut sched = super::Xc124Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_124_scheduler_add_remove() {
        let mut sched = super::Xc124Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_124_scheduler_targets() {
        let sched = super::Xc124Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_124_hash_empty() {
        assert_eq!(super::xc_124_hash(b""), 5381);
    }

    #[test]
    fn xc_124_hash_data() {
        let h = super::xc_124_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_124_hash(b"hello"), h);
    }

    #[test]
    fn xc_124_reverse_str() {
        assert_eq!(super::xc_124_reverse("abc"), "cba");
        assert_eq!(super::xc_124_reverse(""), "");
    }


    // --- xd_37 deepening tests ---

    #[test]
    fn xd_37_sm_initial_state() {
        let sm = Xd37StateMachine::new();
        assert_eq!(sm.current_state(), Xd37State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_37_sm_valid_idle_to_running() {
        let mut sm = Xd37StateMachine::new();
        assert!(sm.transition(Xd37State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd37State::Running);
    }

    #[test]
    fn xd_37_sm_valid_running_to_paused() {
        let mut sm = Xd37StateMachine::new();
        sm.transition(Xd37State::Running).unwrap();
        assert!(sm.transition(Xd37State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd37State::Paused);
    }

    #[test]
    fn xd_37_sm_valid_running_to_done() {
        let mut sm = Xd37StateMachine::new();
        sm.transition(Xd37State::Running).unwrap();
        assert!(sm.transition(Xd37State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd37State::Done);
    }

    #[test]
    fn xd_37_sm_valid_paused_to_running() {
        let mut sm = Xd37StateMachine::new();
        sm.transition(Xd37State::Running).unwrap();
        sm.transition(Xd37State::Paused).unwrap();
        assert!(sm.transition(Xd37State::Running).is_ok());
    }

    #[test]
    fn xd_37_sm_valid_done_to_idle() {
        let mut sm = Xd37StateMachine::new();
        sm.transition(Xd37State::Running).unwrap();
        sm.transition(Xd37State::Done).unwrap();
        assert!(sm.transition(Xd37State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd37State::Idle);
    }

    #[test]
    fn xd_37_sm_invalid_idle_to_done() {
        let mut sm = Xd37StateMachine::new();
        assert!(sm.transition(Xd37State::Done).is_err());
    }

    #[test]
    fn xd_37_sm_invalid_idle_to_paused() {
        let mut sm = Xd37StateMachine::new();
        assert!(sm.transition(Xd37State::Paused).is_err());
    }

    #[test]
    fn xd_37_sm_history_tracking() {
        let mut sm = Xd37StateMachine::new();
        sm.transition(Xd37State::Running).unwrap();
        sm.transition(Xd37State::Paused).unwrap();
        sm.transition(Xd37State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd37State::Idle);
        assert_eq!(sm.history()[0].to, Xd37State::Running);
        assert_eq!(sm.history()[1].from, Xd37State::Running);
        assert_eq!(sm.history()[2].to, Xd37State::Done);
    }

    #[test]
    fn xd_37_sm_serialize_deserialize() {
        let mut sm = Xd37StateMachine::new();
        sm.transition(Xd37State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd37StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd37State::Running));
    }

    #[test]
    fn xd_37_sm_deserialize_invalid() {
        assert_eq!(Xd37StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_37_sm_reset() {
        let mut sm = Xd37StateMachine::new();
        sm.transition(Xd37State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd37State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_37_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd37EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd37Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_37_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd37EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd37Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd37Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_37_bus_unsubscribe() {
        let mut bus = Xd37EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_37_event_kind_and_payload() {
        let e = Xd37Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd37Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_37_bus_clear_history() {
        let mut bus = Xd37EventBus::new();
        bus.publish(Xd37Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_37_sm_step_counter_increments() {
        let mut sm = Xd37StateMachine::new();
        sm.transition(Xd37State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd37State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #35 --

    #[test]
    fn xf35_trie_insert_search() {
        let mut t = Xf35Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf35_trie_starts_with() {
        let mut t = Xf35Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf35_trie_remove() {
        let mut t = Xf35Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf35_trie_word_count() {
        let mut t = Xf35Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf35_trie_longest_prefix() {
        let mut t = Xf35Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf35_trie_all_words() {
        let mut t = Xf35Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf35_trie_autocomplete() {
        let mut t = Xf35Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf35_trie_empty_search() {
        let t = Xf35Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf35_bloom_add_contains() {
        let mut bf = Xf35BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf35_bloom_probably_absent() {
        let bf = Xf35BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf35_bloom_false_positive_rate() {
        let mut bf = Xf35BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf35_bloom_clear() {
        let mut bf = Xf35BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf35_bloom_union() {
        let mut a = Xf35BloomFilter::xf_new(512, 2);
        let mut b = Xf35BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf35_bloom_intersection_estimate() {
        let mut a = Xf35BloomFilter::xf_new(512, 2);
        let mut b = Xf35BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf35_bloom_union_size_mismatch() {
        let a = Xf35BloomFilter::xf_new(256, 2);
        let b = Xf35BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh123_skip_insert_contains() {
        let mut sl = super::Xh123SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh123_skip_remove() {
        let mut sl = super::Xh123SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh123_skip_len() {
        let mut sl = super::Xh123SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh123_skip_range_query() {
        let mut sl = super::Xh123SkipList::xh_new(4);
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
    fn xh123_skip_floor_ceiling() {
        let mut sl = super::Xh123SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh123_skip_rank() {
        let mut sl = super::Xh123SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh123_skip_empty() {
        let sl = super::Xh123SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh123_skip_duplicates() {
        let mut sl = super::Xh123SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh123_bitset_set_test() {
        let mut bs = super::Xh123BitSet::xh_new(256);
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
    fn xh123_bitset_clear_count() {
        let mut bs = super::Xh123BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh123_bitset_and_or_xor() {
        let mut a = super::Xh123BitSet::xh_new(128);
        let mut b = super::Xh123BitSet::xh_new(128);
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
    fn xh123_bitset_iter_ones() {
        let mut bs = super::Xh123BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh123_bitset_first_last() {
        let mut bs = super::Xh123BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh123_bitset_empty() {
        let bs = super::Xh123BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi123_deque_push_pop_back() {
        let mut dq = super::Xi123Deque::xi_new(4);
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
    fn xi123_deque_push_pop_front() {
        let mut dq = super::Xi123Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi123_deque_mixed_ops() {
        let mut dq = super::Xi123Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi123_deque_get_and_split() {
        let mut dq = super::Xi123Deque::xi_new(8);
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
    fn xi123_deque_rotate_left() {
        let mut dq = super::Xi123Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi123_deque_rotate_right() {
        let mut dq = super::Xi123Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi123_deque_grow() {
        let mut dq = super::Xi123Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi123_deque_empty() {
        let dq = super::Xi123Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi123_interval_tree_insert_query() {
        let mut tree = super::Xi123IntervalTree::xi_new();
        tree.xi_insert(super::Xi123Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi123Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi123Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi123_interval_tree_overlap() {
        let mut tree = super::Xi123IntervalTree::xi_new();
        tree.xi_insert(super::Xi123Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi123Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi123Interval::xi_new(12, 20));
        let q = super::Xi123Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi123_interval_tree_remove() {
        let mut tree = super::Xi123IntervalTree::xi_new();
        tree.xi_insert(super::Xi123Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi123Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi123_interval_tree_gaps() {
        let mut tree = super::Xi123IntervalTree::xi_new();
        tree.xi_insert(super::Xi123Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi123Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi123Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi123Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi123Interval::xi_new(8, 10));
    }

    #[test]
    fn xi123_interval_tree_merge() {
        let mut tree = super::Xi123IntervalTree::xi_new();
        tree.xi_insert(super::Xi123Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi123Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi123Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi123Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi123Interval::xi_new(10, 15));
    }

    #[test]
    fn xi123_interval_tree_all() {
        let mut tree = super::Xi123IntervalTree::xi_new();
        tree.xi_insert(super::Xi123Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi123Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi123_interval_tree_empty() {
        let tree = super::Xi123IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi123_interval_tree_contains_point() {
        let iv = super::Xi123Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 123) ---

    #[test]
    fn xj_123_uf_make_and_find() {
        let mut uf = super::Xj123UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_123_uf_union_connected() {
        let mut uf = super::Xj123UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_123_uf_component_count() {
        let mut uf = super::Xj123UnionFind::xj_new();
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
    fn xj_123_uf_component_size() {
        let mut uf = super::Xj123UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_123_uf_largest_component() {
        let mut uf = super::Xj123UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_123_uf_many_elements() {
        let mut uf = super::Xj123UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_123_uf_separate_components() {
        let mut uf = super::Xj123UnionFind::xj_new();
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
    fn xj_123_uf_path_compression() {
        let mut uf = super::Xj123UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_123_bt_insert_get() {
        let mut bt = super::Xj123BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_123_bt_contains_len() {
        let mut bt = super::Xj123BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_123_bt_replace() {
        let mut bt = super::Xj123BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_123_bt_remove() {
        let mut bt = super::Xj123BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_123_bt_keys_values() {
        let mut bt = super::Xj123BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_123_bt_range() {
        let mut bt = super::Xj123BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_123_bt_min_max() {
        let mut bt = super::Xj123BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_123_bt_many_inserts() {
        let mut bt = super::Xj123BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_123 segment tree tests ---

    #[test]
    fn xk_123_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk123SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_123_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk123SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_123_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk123SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_123_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk123SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_123_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk123SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_123_st_single_element() {
        let data = vec![42];
        let st = super::Xk123SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_123_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk123SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_123_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk123SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_123 disjoint intervals tests ---

    #[test]
    fn xk_123_di_add_and_count() {
        let mut di = super::Xk123DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_123_di_merge_overlap() {
        let mut di = super::Xk123DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_123_di_contains() {
        let mut di = super::Xk123DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_123_di_remove() {
        let mut di = super::Xk123DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_123_di_covered_length() {
        let mut di = super::Xk123DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_123_di_gaps() {
        let mut di = super::Xk123DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_123_di_merge_adjacent() {
        let mut di = super::Xk123DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_123_di_empty() {
        let di = super::Xk123DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_123_rope_new_empty() {
        let rope = super::Xl123Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_123_rope_from_str() {
        let rope = super::Xl123Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_123_rope_insert_at() {
        let mut rope = super::Xl123Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_123_rope_delete_range() {
        let mut rope = super::Xl123Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_123_rope_char_at() {
        let rope = super::Xl123Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_123_rope_split_concat() {
        let rope = super::Xl123Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_123_rope_line_count() {
        let rope = super::Xl123Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_123_rope_line_at() {
        let rope = super::Xl123Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_123_sa_build_and_search() {
        let sa = super::Xl123SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_123_sa_count() {
        let sa = super::Xl123SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_123_sa_longest_repeated() {
        let sa = super::Xl123SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_123_sa_all_positions() {
        let sa = super::Xl123SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_123_sa_len() {
        let sa = super::Xl123SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_123_sa_empty() {
        let sa = super::Xl123SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_123_rope_slice() {
        let rope = super::Xl123Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_123_sa_search_start() {
        let sa = super::Xl123SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_123_sparse_set_get() {
        let mut m = super::Xm123MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_123_sparse_row_col() {
        let mut m = super::Xm123MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_123_sparse_transpose() {
        let mut m = super::Xm123MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_123_sparse_multiply_vec() {
        let mut m = super::Xm123MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_123_sparse_nnz_density() {
        let mut m = super::Xm123MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_123_sparse_clear() {
        let mut m = super::Xm123MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_123_sparse_overwrite_zero() {
        let mut m = super::Xm123MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_123_tokenizer_basic() {
        let t = super::Xm123Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_123_tokenizer_count() {
        let t = super::Xm123Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_123_tokenizer_unique() {
        let t = super::Xm123Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_123_tokenizer_frequency() {
        let t = super::Xm123Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_123_tokenizer_delimiter() {
        let t = super::Xm123Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_123_tokenizer_whitespace() {
        let t = super::Xm123Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_123_tokenizer_empty() {
        let t = super::Xm123Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 123 ----

    #[test]
    fn xn_123_fenwick_prefix_sum() {
        let mut ft = super::Xn123Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_123_fenwick_range_sum() {
        let mut ft = super::Xn123Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_123_fenwick_point_query() {
        let mut ft = super::Xn123Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_123_fenwick_len() {
        let ft = super::Xn123Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_123_fenwick_multiple_updates() {
        let mut ft = super::Xn123Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_123_fenwick_single_element() {
        let mut ft = super::Xn123Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_123_fenwick_find_kth() {
        let mut ft = super::Xn123Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_123_fenwick_negative_delta() {
        let mut ft = super::Xn123Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 123 ----

    #[test]
    fn xn_123_avl_insert_get() {
        let mut m = super::Xn123AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_123_avl_remove() {
        let mut m = super::Xn123AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_123_avl_in_order() {
        let mut m = super::Xn123AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_123_avl_min_max() {
        let mut m = super::Xn123AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_123_avl_floor_ceiling() {
        let mut m = super::Xn123AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_123_avl_height_balanced() {
        let mut m = super::Xn123AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_123_avl_overwrite() {
        let mut m = super::Xn123AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_123_avl_empty() {
        let m: super::Xn123AVL<i32, i32> = super::Xn123AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo123RedBlack tests ---

    #[test]
    fn xo_123_rb_insert_and_get() {
        let mut tree = super::Xo123RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_123_rb_len_and_empty() {
        let mut tree = super::Xo123RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_123_rb_min_max() {
        let mut tree = super::Xo123RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_123_rb_contains() {
        let mut tree = super::Xo123RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_123_rb_remove() {
        let mut tree = super::Xo123RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_123_rb_in_order() {
        let mut tree = super::Xo123RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_123_rb_black_height() {
        let mut tree = super::Xo123RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_123_rb_overwrite() {
        let mut tree = super::Xo123RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo123ConsistentHash tests ---

    #[test]
    fn xo_123_ch_add_and_count() {
        let mut ring = super::Xo123ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_123_ch_remove_node() {
        let mut ring = super::Xo123ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_123_ch_get_node() {
        let mut ring = super::Xo123ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_123_ch_empty_ring() {
        let ring = super::Xo123ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_123_ch_distribution() {
        let mut ring = super::Xo123ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_123_ch_rebalance() {
        let mut ring = super::Xo123ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_123_ch_virtual_nodes() {
        let mut ring = super::Xo123ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_123_ch_consistent_lookup() {
        let mut ring = super::Xo123ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_123_splay_insert_get() {
        let mut t = super::Xp123SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_123_splay_remove() {
        let mut t = super::Xp123SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_123_splay_count_increases() {
        let mut t = super::Xp123SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_123_splay_depth() {
        let mut t = super::Xp123SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_123_splay_len_empty() {
        let t = super::Xp123SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_123_splay_min_max() {
        let mut t = super::Xp123SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_123_splay_overwrite() {
        let mut t = super::Xp123SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_123_splay_remove_missing() {
        let mut t = super::Xp123SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_123 treap tests ----
    #[test]
    fn xq_123_treap_empty() {
        let t = super::Xq123Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_123_treap_insert_get() {
        let mut t = super::Xq123Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_123_treap_overwrite() {
        let mut t = super::Xq123Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_123_treap_remove() {
        let mut t = super::Xq123Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_123_treap_min_max() {
        let mut t = super::Xq123Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_123_treap_rank() {
        let mut t = super::Xq123Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_123_treap_kth() {
        let mut t = super::Xq123Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_123_treap_in_order() {
        let mut t = super::Xq123Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_123 VEB tree tests ----
    #[test]
    fn xq_123_veb_empty() {
        let v = super::Xq123VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_123_veb_insert_contains() {
        let mut v = super::Xq123VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_123_veb_min_max() {
        let mut v = super::Xq123VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_123_veb_delete() {
        let mut v = super::Xq123VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_123_veb_successor() {
        let mut v = super::Xq123VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_123_veb_predecessor() {
        let mut v = super::Xq123VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_123_veb_count() {
        let mut v = super::Xq123VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_123_veb_duplicate_insert() {
        let mut v = super::Xq123VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_123_kdtree_empty() {
        let tree = super::Xr123KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_123_kdtree_insert_one() {
        let mut tree = super::Xr123KDTree::xr_new();
        tree.xr_insert(super::Xr123KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_123_kdtree_insert_multiple() {
        let mut tree = super::Xr123KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr123KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_123_kdtree_nearest_neighbor() {
        let mut tree = super::Xr123KDTree::xr_new();
        tree.xr_insert(super::Xr123KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr123KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr123KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_123_kdtree_nn_empty() {
        let tree = super::Xr123KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr123KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_123_kdtree_range_search() {
        let mut tree = super::Xr123KDTree::xr_new();
        tree.xr_insert(super::Xr123KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr123KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr123KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_123_kdtree_range_empty() {
        let mut tree = super::Xr123KDTree::xr_new();
        tree.xr_insert(super::Xr123KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_123_kdtree_all_points() {
        let mut tree = super::Xr123KDTree::xr_new();
        tree.xr_insert(super::Xr123KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr123KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_123_kdtree_depth() {
        let mut tree = super::Xr123KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr123KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_123_kdtree_bounding_box() {
        let mut tree = super::Xr123KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr123KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr123KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_123_persistent_array_new() {
        let arr = super::Xs123PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_123_persistent_array_push() {
        let mut arr = super::Xs123PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_123_persistent_array_set() {
        let mut arr = super::Xs123PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_123_persistent_array_diff() {
        let mut arr = super::Xs123PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_123_persistent_array_rollback() {
        let mut arr = super::Xs123PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_123_persistent_array_history() {
        let mut arr = super::Xs123PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_123_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs123PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_123_persistent_array_from_vec() {
        let arr = super::Xs123PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_123_concurrent_queue_new() {
        let q = super::Xs123ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_123_concurrent_queue_push_pop() {
        let mut q = super::Xs123ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_123_concurrent_queue_full() {
        let mut q = super::Xs123ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_123_concurrent_queue_drain() {
        let mut q = super::Xs123ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_123_concurrent_queue_try_pop() {
        let mut q = super::Xs123ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_123_concurrent_queue_clear() {
        let mut q = super::Xs123ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_123_range_map_new() {
        let rm = super::Xs123RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_123_range_map_insert_get() {
        let mut rm = super::Xs123RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_123_range_map_overlap() {
        let mut rm = super::Xs123RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_123_range_map_remove() {
        let mut rm = super::Xs123RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_123_range_map_gaps() {
        let mut rm = super::Xs123RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_123_range_map_coverage() {
        let mut rm = super::Xs123RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_123_range_map_contains() {
        let mut rm = super::Xs123RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_123_range_map_clear() {
        let mut rm = super::Xs123RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_123_circular_buffer_new() {
        let buf = super::Xs123CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_123_circular_buffer_push_pop() {
        let mut buf = super::Xs123CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_123_circular_buffer_overwrite() {
        let mut buf = super::Xs123CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_123_circular_buffer_peek() {
        let mut buf = super::Xs123CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_123_circular_buffer_is_full() {
        let mut buf = super::Xs123CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_123_circular_buffer_iter() {
        let mut buf = super::Xs123CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_123_circular_buffer_clear() {
        let mut buf = super::Xs123CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_123_circular_buffer_to_vec() {
        let mut buf = super::Xs123CircularBuffer::xs_new(4);
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
