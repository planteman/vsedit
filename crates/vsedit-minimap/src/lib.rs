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
}
