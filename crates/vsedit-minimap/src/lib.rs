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
}
