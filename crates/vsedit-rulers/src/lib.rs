//! Column ruler lines.

/// A single ruler at a specific column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulerConfig {
    pub column: u32,
    pub color: Option<String>,
}

/// Configuration for all editor rulers.
#[derive(Debug, Clone)]
pub struct RulersConfig {
    pub rulers: Vec<RulerConfig>,
    pub default_color: String,
}

impl Default for RulersConfig {
    fn default() -> Self {
        Self {
            rulers: Vec::new(),
            default_color: "#d3d3d3".to_string(),
        }
    }
}

impl RulersConfig {
    /// Add a ruler at the given column with an optional color override.
    pub fn add_ruler(&mut self, column: u32, color: Option<String>) {
        self.rulers.push(RulerConfig { column, color });
    }

    /// Remove all rulers at the given column. Returns the number removed.
    pub fn remove_ruler(&mut self, column: u32) -> usize {
        let before = self.rulers.len();
        self.rulers.retain(|r| r.column != column);
        before - self.rulers.len()
    }

    /// Return a slice of all configured rulers.
    pub fn get_rulers(&self) -> &[RulerConfig] {
        &self.rulers
    }

    /// Check whether any ruler exists at the given column.
    pub fn has_ruler_at(&self, column: u32) -> bool {
        self.rulers.iter().any(|r| r.column == column)
    }

    /// Return rulers sorted by column position.
    pub fn sorted_rulers(&self) -> Vec<&RulerConfig> {
        let mut sorted: Vec<&RulerConfig> = self.rulers.iter().collect();
        sorted.sort_by_key(|r| r.column);
        sorted
    }
}

/// Service that computes ruler line positions from a `RulersConfig`.
pub struct RulerService {
    config: RulersConfig,
}

impl RulerService {
    pub fn new(config: RulersConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &RulersConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut RulersConfig {
        &mut self.config
    }

    /// Compute pixel x-positions for ruler lines given a character width.
    pub fn compute_positions(&self, char_width: f64) -> Vec<RulerPosition> {
        self.config
            .rulers
            .iter()
            .map(|r| {
                let color = r
                    .color
                    .clone()
                    .unwrap_or_else(|| self.config.default_color.clone());
                RulerPosition {
                    column: r.column,
                    x: r.column as f64 * char_width,
                    color,
                }
            })
            .collect()
    }

    /// Return only rulers whose column falls within `[0, visible_columns)`.
    pub fn visible_rulers(&self, visible_columns: u32) -> Vec<&RulerConfig> {
        self.config
            .rulers
            .iter()
            .filter(|r| r.column < visible_columns)
            .collect()
    }
}

/// A computed ruler position ready for rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerPosition {
    pub column: u32,
    pub x: f64,
    pub color: String,
}

/// An overlay decoration representing a single ruler line.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerDecoration {
    pub x: f64,
    pub height: f64,
    pub color: String,
    pub width: f64,
}

/// Generate overlay decorations from ruler positions.
pub fn render_rulers(
    positions: &[RulerPosition],
    viewport_height: f64,
    line_width: f64,
) -> Vec<RulerDecoration> {
    positions
        .iter()
        .map(|pos| RulerDecoration {
            x: pos.x,
            height: viewport_height,
            color: pos.color.clone(),
            width: line_width,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = RulersConfig::default();
        assert!(cfg.rulers.is_empty());
        assert_eq!(cfg.default_color, "#d3d3d3");
    }

    #[test]
    fn add_and_query_rulers() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, Some("#ff0000".to_string()));
        assert_eq!(cfg.get_rulers().len(), 2);
        assert!(cfg.has_ruler_at(80));
        assert!(cfg.has_ruler_at(120));
        assert!(!cfg.has_ruler_at(100));
    }

    #[test]
    fn remove_ruler() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, None);
        assert_eq!(cfg.remove_ruler(80), 1);
        assert!(!cfg.has_ruler_at(80));
        assert!(cfg.has_ruler_at(120));
        assert_eq!(cfg.remove_ruler(999), 0);
    }

    #[test]
    fn sorted_rulers_returns_ordered() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(120, None);
        cfg.add_ruler(40, None);
        cfg.add_ruler(80, None);
        let sorted = cfg.sorted_rulers();
        assert_eq!(sorted[0].column, 40);
        assert_eq!(sorted[1].column, 80);
        assert_eq!(sorted[2].column, 120);
    }

    #[test]
    fn compute_positions_with_defaults() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, Some("#ff0000".into()));
        let svc = RulerService::new(cfg);
        let positions = svc.compute_positions(8.0);
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].column, 80);
        assert!((positions[0].x - 640.0).abs() < f64::EPSILON);
        assert_eq!(positions[0].color, "#d3d3d3");
        assert_eq!(positions[1].color, "#ff0000");
    }

    #[test]
    fn visible_rulers_filters_by_viewport() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(40, None);
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, None);
        let svc = RulerService::new(cfg);
        let visible = svc.visible_rulers(100);
        assert_eq!(visible.len(), 2);
        assert!(visible.iter().all(|r| r.column < 100));
    }

    #[test]
    fn render_rulers_produces_decorations() {
        let positions = vec![
            RulerPosition { column: 80, x: 640.0, color: "#aaa".into() },
            RulerPosition { column: 120, x: 960.0, color: "#bbb".into() },
        ];
        let decorations = render_rulers(&positions, 500.0, 1.0);
        assert_eq!(decorations.len(), 2);
        assert!((decorations[0].x - 640.0).abs() < f64::EPSILON);
        assert!((decorations[0].height - 500.0).abs() < f64::EPSILON);
        assert!((decorations[0].width - 1.0).abs() < f64::EPSILON);
        assert_eq!(decorations[1].color, "#bbb");
    }

    #[test]
    fn render_rulers_empty() {
        let decorations = render_rulers(&[], 500.0, 1.0);
        assert!(decorations.is_empty());
    }
}
