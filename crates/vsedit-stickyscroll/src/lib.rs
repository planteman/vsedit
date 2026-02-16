//! Sticky scroll widget that pins nesting-context lines at the top of the editor viewport.

use std::fmt;

/// Errors produced by [`StickyScrollWidget`] operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StickyScrollError {
    WidgetDisabled,
    NoLinesAvailable,
    InvalidNestingLevel(u32),
}

impl fmt::Display for StickyScrollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WidgetDisabled => write!(f, "sticky scroll widget is disabled"),
            Self::NoLinesAvailable => write!(f, "no sticky scroll lines available"),
            Self::InvalidNestingLevel(n) => write!(f, "invalid nesting level: {n}"),
        }
    }
}

/// A single line pinned in the sticky scroll area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyScrollLine {
    pub line_number: u32,
    pub text: String,
    pub nesting_level: u32,
    pub collapsed: bool,
}

impl StickyScrollLine {
    /// Return a string of spaces proportional to `nesting_level` (4 spaces per level).
    pub fn indentation(&self) -> String {
        " ".repeat(self.nesting_level as usize * 4)
    }
}

impl fmt::Display for StickyScrollLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "L{}: {} (level {})",
            self.line_number, self.text, self.nesting_level
        )
    }
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

    /// Toggle the collapsed state of the sticky line at the given line number.
    pub fn toggle_collapse(&mut self, line_number: u32) -> Result<(), StickyScrollError> {
        if !self.enabled {
            return Err(StickyScrollError::WidgetDisabled);
        }
        let line = self
            .lines
            .iter_mut()
            .find(|l| l.line_number == line_number)
            .ok_or(StickyScrollError::NoLinesAvailable)?;
        line.collapsed = !line.collapsed;
        Ok(())
    }

    /// Return the number of currently collapsed sticky lines.
    pub fn collapsed_count(&self) -> usize {
        self.lines.iter().filter(|l| l.collapsed).count()
    }

    /// Return the highest nesting level among current sticky lines, or `None` if empty.
    pub fn max_nesting_level(&self) -> Option<u32> {
        self.lines.iter().map(|l| l.nesting_level).max()
    }

    /// Find a sticky line by its line number.
    pub fn find_by_line(&self, line_number: u32) -> Option<&StickyScrollLine> {
        self.lines.iter().find(|l| l.line_number == line_number)
    }

    /// Dynamically change the maximum number of displayed sticky lines.
    pub fn set_max_lines(&mut self, max_lines: usize) {
        self.max_lines = max_lines;
        self.lines.truncate(max_lines);
    }

    /// Return only the non-collapsed sticky lines.
    pub fn get_uncollapsed_lines(&self) -> Vec<&StickyScrollLine> {
        self.lines.iter().filter(|l| !l.collapsed).collect()
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

/// Builder for [`StickyScrollConfig`].
#[derive(Debug, Clone)]
pub struct StickyScrollConfigBuilder {
    enabled: bool,
    max_line_count: usize,
    default_model: String,
}

impl StickyScrollConfigBuilder {
    pub fn new() -> Self {
        let defaults = StickyScrollConfig::default();
        Self {
            enabled: defaults.enabled,
            max_line_count: defaults.max_line_count,
            default_model: defaults.default_model,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn max_line_count(mut self, count: usize) -> Self {
        self.max_line_count = count;
        self
    }

    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    pub fn build(self) -> StickyScrollConfig {
        StickyScrollConfig {
            enabled: self.enabled,
            max_line_count: self.max_line_count,
            default_model: self.default_model,
        }
    }
}

impl Default for StickyScrollConfigBuilder {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn toggle_collapse_works() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "fn main() {", 0), (5, "if cond {", 1)]);
        assert!(!w.find_by_line(1).unwrap().collapsed);
        w.toggle_collapse(1).unwrap();
        assert!(w.find_by_line(1).unwrap().collapsed);
        w.toggle_collapse(1).unwrap();
        assert!(!w.find_by_line(1).unwrap().collapsed);
    }

    #[test]
    fn toggle_collapse_disabled_widget() {
        let mut w = StickyScrollWidget::new(5);
        w.set_enabled(false);
        assert_eq!(w.toggle_collapse(1), Err(StickyScrollError::WidgetDisabled));
    }

    #[test]
    fn toggle_collapse_missing_line() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 10, &[(1, "fn foo() {", 0)]);
        assert_eq!(
            w.toggle_collapse(99),
            Err(StickyScrollError::NoLinesAvailable)
        );
    }

    #[test]
    fn collapsed_count_tracks_collapsed() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0), (2, "b", 1), (3, "c", 2)]);
        assert_eq!(w.collapsed_count(), 0);
        w.toggle_collapse(1).unwrap();
        w.toggle_collapse(3).unwrap();
        assert_eq!(w.collapsed_count(), 2);
    }

    #[test]
    fn max_nesting_level_works() {
        let mut w = StickyScrollWidget::new(10);
        assert_eq!(w.max_nesting_level(), None);
        w.update_lines(0, 30, &[(1, "a", 0), (5, "b", 3), (10, "c", 2)]);
        assert_eq!(w.max_nesting_level(), Some(3));
    }

    #[test]
    fn find_by_line_returns_correct() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "fn main() {", 0), (5, "if cond {", 1)]);
        let found = w.find_by_line(5).unwrap();
        assert_eq!(found.text, "if cond {");
        assert!(w.find_by_line(99).is_none());
    }

    #[test]
    fn set_max_lines_truncates() {
        let mut w = StickyScrollWidget::new(10);
        w.update_lines(0, 30, &[(1, "a", 0), (2, "b", 1), (3, "c", 2), (4, "d", 3)]);
        assert_eq!(w.get_visible_sticky_lines().len(), 4);
        w.set_max_lines(2);
        assert_eq!(w.get_visible_sticky_lines().len(), 2);
    }

    #[test]
    fn display_impl_for_line() {
        let line = StickyScrollLine {
            line_number: 42,
            text: "fn hello()".to_string(),
            nesting_level: 2,
            collapsed: false,
        };
        assert_eq!(format!("{line}"), "L42: fn hello() (level 2)");
    }

    #[test]
    fn indentation_proportional_to_level() {
        let line = StickyScrollLine {
            line_number: 1,
            text: "x".to_string(),
            nesting_level: 3,
            collapsed: false,
        };
        assert_eq!(line.indentation(), "            "); // 12 spaces
        let root = StickyScrollLine {
            line_number: 1,
            text: "y".to_string(),
            nesting_level: 0,
            collapsed: false,
        };
        assert_eq!(root.indentation(), "");
    }

    #[test]
    fn config_builder_defaults() {
        let cfg = StickyScrollConfigBuilder::new().build();
        assert_eq!(cfg, StickyScrollConfig::default());
    }

    #[test]
    fn config_builder_custom() {
        let cfg = StickyScrollConfigBuilder::new()
            .enabled(false)
            .max_line_count(10)
            .default_model("foldingModel")
            .build();
        assert!(!cfg.enabled);
        assert_eq!(cfg.max_line_count, 10);
        assert_eq!(cfg.default_model, "foldingModel");
    }

    #[test]
    fn get_uncollapsed_lines_filters() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0), (2, "b", 1), (3, "c", 2)]);
        w.toggle_collapse(2).unwrap();
        let uncollapsed = w.get_uncollapsed_lines();
        assert_eq!(uncollapsed.len(), 2);
        assert_eq!(uncollapsed[0].line_number, 1);
        assert_eq!(uncollapsed[1].line_number, 3);
    }

    #[test]
    fn error_display_messages() {
        assert_eq!(
            format!("{}", StickyScrollError::WidgetDisabled),
            "sticky scroll widget is disabled"
        );
        assert_eq!(
            format!("{}", StickyScrollError::NoLinesAvailable),
            "no sticky scroll lines available"
        );
        assert_eq!(
            format!("{}", StickyScrollError::InvalidNestingLevel(7)),
            "invalid nesting level: 7"
        );
    }
}
