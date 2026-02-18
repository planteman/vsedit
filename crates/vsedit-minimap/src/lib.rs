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

}
