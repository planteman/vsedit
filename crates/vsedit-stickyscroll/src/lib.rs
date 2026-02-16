//! Sticky scroll widget that pins nesting-context lines at the top of the editor viewport.

/// A single line pinned in the sticky scroll area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyScrollLine {
    pub line_number: u32,
    pub text: String,
    pub nesting_level: u32,
    pub collapsed: bool,
}

/// Widget that manages the set of sticky scroll lines.
#[derive(Debug, Clone)]
pub struct StickyScrollWidget {
    lines: Vec<StickyScrollLine>,
    max_lines: usize,
    enabled: bool,
}

impl StickyScrollWidget {
    /// Create a new widget that displays at most `max_lines` sticky lines.
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: Vec::new(),
            max_lines,
            enabled: true,
        }
    }

    /// Replace the current sticky lines based on visible ranges and nesting data.
    ///
    /// `nesting_data` entries are `(line_number, text, nesting_level)` tuples that
    /// describe scope-opening lines (e.g. function / class headers). Only lines
    /// whose nesting level fits within `max_lines` are retained.
    pub fn update_lines(
        &mut self,
        _visible_range_start: u32,
        _visible_range_end: u32,
        nesting_data: &[(u32, &str, u32)],
    ) {
        if !self.enabled {
            return;
        }
        self.lines = nesting_data
            .iter()
            .take(self.max_lines)
            .map(|(line, text, level)| StickyScrollLine {
                line_number: *line,
                text: (*text).to_string(),
                nesting_level: *level,
                collapsed: false,
            })
            .collect();
    }

    /// Enable or disable the widget.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.lines.clear();
        }
    }

    /// Return the currently visible sticky scroll lines.
    pub fn get_visible_sticky_lines(&self) -> &[StickyScrollLine] {
        &self.lines
    }

    /// Remove all sticky lines.
    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

/// Configuration for the sticky scroll feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyScrollConfig {
    pub enabled: bool,
    pub max_line_count: usize,
    pub default_model: String,
}

impl Default for StickyScrollConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_line_count: 5,
            default_model: "outlineModel".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_widget_is_empty_and_enabled() {
        let w = StickyScrollWidget::new(5);
        assert!(w.enabled);
        assert!(w.get_visible_sticky_lines().is_empty());
    }

    #[test]
    fn update_lines_respects_max() {
        let mut w = StickyScrollWidget::new(2);
        let data = vec![
            (1, "fn main() {", 0),
            (5, "if cond {", 1),
            (10, "for x in xs {", 2),
        ];
        w.update_lines(0, 20, &data);
        assert_eq!(w.get_visible_sticky_lines().len(), 2);
        assert_eq!(w.get_visible_sticky_lines()[0].line_number, 1);
        assert_eq!(w.get_visible_sticky_lines()[1].line_number, 5);
    }

    #[test]
    fn disable_clears_lines() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 10, &[(1, "fn foo() {", 0)]);
        assert_eq!(w.get_visible_sticky_lines().len(), 1);

        w.set_enabled(false);
        assert!(w.get_visible_sticky_lines().is_empty());

        // update_lines is a no-op when disabled
        w.update_lines(0, 10, &[(2, "fn bar() {", 0)]);
        assert!(w.get_visible_sticky_lines().is_empty());
    }

    #[test]
    fn default_config() {
        let cfg = StickyScrollConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.max_line_count, 5);
        assert_eq!(cfg.default_model, "outlineModel");
    }
}
