//! Virtual scrollbar widget.

use std::fmt;

/// Scrollbar orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarOrientation {
    Vertical,
    Horizontal,
}

/// When to show a scrollbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarVisibility {
    Auto,
    Visible,
    Hidden,
}

/// Scrollbar configuration.
#[derive(Debug, Clone)]
pub struct ScrollbarConfig {
    pub vertical: ScrollbarVisibility,
    pub horizontal: ScrollbarVisibility,
    pub scroll_sensitivity: f64,
    pub fast_scroll_sensitivity: f64,
}

impl Default for ScrollbarConfig {
    fn default() -> Self {
        Self {
            vertical: ScrollbarVisibility::Auto,
            horizontal: ScrollbarVisibility::Auto,
            scroll_sensitivity: 1.0,
            fast_scroll_sensitivity: 5.0,
        }
    }
}

/// Current scroll position and viewport dimensions.
#[derive(Debug, Clone)]
pub struct ScrollState {
    pub scroll_top: f64,
    pub scroll_left: f64,
    pub viewport_height: f64,
    pub viewport_width: f64,
    pub content_height: f64,
    pub content_width: f64,
}

impl ScrollState {
    /// Vertical scroll progress as a value in `0.0..=1.0`.
    pub fn scroll_percentage_vertical(&self) -> f64 {
        let max = (self.content_height - self.viewport_height).max(0.0);
        if max == 0.0 { 0.0 } else { (self.scroll_top / max).clamp(0.0, 1.0) }
    }

    /// Horizontal scroll progress as a value in `0.0..=1.0`.
    pub fn scroll_percentage_horizontal(&self) -> f64 {
        let max = (self.content_width - self.viewport_width).max(0.0);
        if max == 0.0 { 0.0 } else { (self.scroll_left / max).clamp(0.0, 1.0) }
    }

    pub fn can_scroll_up(&self) -> bool {
        self.scroll_top > 0.0
    }

    pub fn can_scroll_down(&self) -> bool {
        self.scroll_top + self.viewport_height < self.content_height
    }

    pub fn can_scroll_left(&self) -> bool {
        self.scroll_left > 0.0
    }

    pub fn can_scroll_right(&self) -> bool {
        self.scroll_left + self.viewport_width < self.content_width
    }

    /// Clamp scroll offsets so they stay within valid bounds.
    pub fn clamp_scroll(&mut self) {
        let max_top = (self.content_height - self.viewport_height).max(0.0);
        let max_left = (self.content_width - self.viewport_width).max(0.0);
        self.scroll_top = self.scroll_top.clamp(0.0, max_top);
        self.scroll_left = self.scroll_left.clamp(0.0, max_left);
    }

    /// Set vertical scroll position from a percentage (`0.0..=1.0`).
    pub fn set_scroll_from_percentage_vertical(&mut self, pct: f64) {
        let pct = pct.clamp(0.0, 1.0);
        let max = (self.content_height - self.viewport_height).max(0.0);
        self.scroll_top = pct * max;
    }

    /// Set horizontal scroll position from a percentage (`0.0..=1.0`).
    pub fn set_scroll_from_percentage_horizontal(&mut self, pct: f64) {
        let pct = pct.clamp(0.0, 1.0);
        let max = (self.content_width - self.viewport_width).max(0.0);
        self.scroll_left = pct * max;
    }
}

impl fmt::Display for ScrollState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ScrollState(V:{:.1}% H:{:.1}%)",
            self.scroll_percentage_vertical() * 100.0,
            self.scroll_percentage_horizontal() * 100.0,
        )
    }
}

/// Errors that can occur when working with scrollbar dimensions.
#[derive(Debug, Clone, PartialEq)]
pub enum ScrollbarError {
    /// A dimension (width/height) was invalid (e.g. negative or NaN).
    InvalidDimension(String),
    /// A scroll position was outside the valid content bounds.
    OutOfBounds { position: f64, max: f64 },
}

impl fmt::Display for ScrollbarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScrollbarError::InvalidDimension(msg) => {
                write!(f, "invalid dimension: {msg}")
            }
            ScrollbarError::OutOfBounds { position, max } => {
                write!(f, "position {position} out of bounds (max {max})")
            }
        }
    }
}

/// A discrete scroll input event.
#[derive(Debug, Clone, Copy)]
pub struct ScrollEvent {
    /// Horizontal scroll delta (positive = right).
    pub delta_x: f64,
    /// Vertical scroll delta (positive = down).
    pub delta_y: f64,
    /// Whether this is a fast/accelerated scroll (e.g. shift held).
    pub is_fast: bool,
    /// Monotonic timestamp in milliseconds.
    pub timestamp: u64,
}

/// Pre-computed metrics for rendering a scrollbar track and thumb.
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarMetrics {
    /// Size of the thumb along the scrollbar axis.
    pub thumb_size: f64,
    /// Position of the thumb along the scrollbar axis.
    pub thumb_position: f64,
}

impl ScrollbarMetrics {
    /// Compute metrics for a vertical scrollbar.
    pub fn vertical(state: &ScrollState, track_length: f64) -> Self {
        let ratio = if state.content_height > 0.0 {
            state.viewport_height / state.content_height
        } else {
            1.0
        };
        let thumb_size = (ratio * track_length).clamp(20.0, track_length);
        let pct = state.scroll_percentage_vertical();
        let thumb_position = pct * (track_length - thumb_size);
        Self { thumb_size, thumb_position }
    }

    /// Compute metrics for a horizontal scrollbar.
    pub fn horizontal(state: &ScrollState, track_length: f64) -> Self {
        let ratio = if state.content_width > 0.0 {
            state.viewport_width / state.content_width
        } else {
            1.0
        };
        let thumb_size = (ratio * track_length).clamp(20.0, track_length);
        let pct = state.scroll_percentage_horizontal();
        let thumb_position = pct * (track_length - thumb_size);
        Self { thumb_size, thumb_position }
    }
}

/// A combined scrollbar widget holding both configuration and state.
#[derive(Debug, Clone)]
pub struct ScrollbarWidget {
    pub config: ScrollbarConfig,
    pub state: ScrollState,
}

impl ScrollbarWidget {
    /// Create a new widget with the given config and state.
    pub fn new(config: ScrollbarConfig, state: ScrollState) -> Self {
        Self { config, state }
    }

    /// Apply a `ScrollEvent` to the current state, respecting sensitivity.
    pub fn apply_scroll_event(&mut self, event: ScrollEvent) {
        let sensitivity = if event.is_fast {
            self.config.fast_scroll_sensitivity
        } else {
            self.config.scroll_sensitivity
        };
        self.state.scroll_top += event.delta_y * sensitivity;
        self.state.scroll_left += event.delta_x * sensitivity;
        self.state.clamp_scroll();
    }

    /// Scroll to the very top of the content.
    pub fn scroll_to_top(&mut self) {
        self.state.scroll_top = 0.0;
    }

    /// Scroll to the very bottom of the content.
    pub fn scroll_to_bottom(&mut self) {
        let max = (self.state.content_height - self.state.viewport_height).max(0.0);
        self.state.scroll_top = max;
    }

    /// Scroll up by one viewport height.
    pub fn scroll_page_up(&mut self) {
        self.state.scroll_top -= self.state.viewport_height;
        self.state.clamp_scroll();
    }

    /// Scroll down by one viewport height.
    pub fn scroll_page_down(&mut self) {
        self.state.scroll_top += self.state.viewport_height;
        self.state.clamp_scroll();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = ScrollbarConfig::default();
        assert_eq!(cfg.vertical, ScrollbarVisibility::Auto);
        assert!((cfg.scroll_sensitivity - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_percentage() {
        let state = ScrollState {
            scroll_top: 50.0,
            scroll_left: 0.0,
            viewport_height: 100.0,
            viewport_width: 200.0,
            content_height: 200.0,
            content_width: 200.0,
        };
        assert!((state.scroll_percentage_vertical() - 0.5).abs() < f64::EPSILON);
        assert!((state.scroll_percentage_horizontal() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn can_scroll_and_clamp() {
        let mut state = ScrollState {
            scroll_top: 250.0,
            scroll_left: -10.0,
            viewport_height: 100.0,
            viewport_width: 100.0,
            content_height: 300.0,
            content_width: 100.0,
        };
        assert!(state.can_scroll_up());
        assert!(!state.can_scroll_down()); // 250 + 100 > 300
        // scroll_left is negative so viewport hasn't reached the right edge
        assert!(state.can_scroll_right());

        state.clamp_scroll();
        assert!((state.scroll_top - 200.0).abs() < f64::EPSILON);
        assert!((state.scroll_left - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scrollbar_error_display() {
        let e1 = ScrollbarError::InvalidDimension("negative width".into());
        assert_eq!(format!("{e1}"), "invalid dimension: negative width");

        let e2 = ScrollbarError::OutOfBounds { position: 500.0, max: 200.0 };
        assert_eq!(format!("{e2}"), "position 500 out of bounds (max 200)");
    }

    #[test]
    fn scroll_state_display() {
        let state = ScrollState {
            scroll_top: 50.0,
            scroll_left: 25.0,
            viewport_height: 100.0,
            viewport_width: 100.0,
            content_height: 200.0,
            content_width: 200.0,
        };
        assert_eq!(format!("{state}"), "ScrollState(V:50.0% H:25.0%)");
    }

    #[test]
    fn set_scroll_from_percentage() {
        let mut state = ScrollState {
            scroll_top: 0.0,
            scroll_left: 0.0,
            viewport_height: 100.0,
            viewport_width: 100.0,
            content_height: 500.0,
            content_width: 300.0,
        };
        state.set_scroll_from_percentage_vertical(0.5);
        assert!((state.scroll_top - 200.0).abs() < f64::EPSILON);

        state.set_scroll_from_percentage_horizontal(1.0);
        assert!((state.scroll_left - 200.0).abs() < f64::EPSILON);

        // Clamps values outside 0..=1
        state.set_scroll_from_percentage_vertical(2.0);
        assert!((state.scroll_top - 400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scrollbar_metrics_vertical() {
        let state = ScrollState {
            scroll_top: 100.0,
            scroll_left: 0.0,
            viewport_height: 100.0,
            viewport_width: 200.0,
            content_height: 400.0,
            content_width: 200.0,
        };
        let m = ScrollbarMetrics::vertical(&state, 300.0);
        // ratio = 100/400 = 0.25, thumb_size = 75
        assert!((m.thumb_size - 75.0).abs() < f64::EPSILON);
        // pct = 100/300 = 1/3, position = (1/3)*(300-75)
        let expected_pos = (1.0 / 3.0) * 225.0;
        assert!((m.thumb_position - expected_pos).abs() < 0.01);
    }

    #[test]
    fn scrollbar_metrics_horizontal() {
        let state = ScrollState {
            scroll_top: 0.0,
            scroll_left: 0.0,
            viewport_height: 100.0,
            viewport_width: 200.0,
            content_height: 100.0,
            content_width: 200.0,
        };
        let m = ScrollbarMetrics::horizontal(&state, 400.0);
        // content_width == viewport_width, ratio=1.0, thumb fills track
        assert!((m.thumb_size - 400.0).abs() < f64::EPSILON);
        assert!((m.thumb_position - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn widget_apply_scroll_event() {
        let mut w = ScrollbarWidget::new(
            ScrollbarConfig::default(),
            ScrollState {
                scroll_top: 0.0,
                scroll_left: 0.0,
                viewport_height: 100.0,
                viewport_width: 100.0,
                content_height: 500.0,
                content_width: 500.0,
            },
        );
        let evt = ScrollEvent { delta_x: 10.0, delta_y: 20.0, is_fast: false, timestamp: 0 };
        w.apply_scroll_event(evt);
        assert!((w.state.scroll_top - 20.0).abs() < f64::EPSILON);
        assert!((w.state.scroll_left - 10.0).abs() < f64::EPSILON);

        // Fast scroll uses fast_scroll_sensitivity (5.0)
        let fast = ScrollEvent { delta_x: 0.0, delta_y: 10.0, is_fast: true, timestamp: 1 };
        w.apply_scroll_event(fast);
        assert!((w.state.scroll_top - 70.0).abs() < f64::EPSILON);
    }

    #[test]
    fn widget_scroll_to_top_bottom() {
        let mut w = ScrollbarWidget::new(
            ScrollbarConfig::default(),
            ScrollState {
                scroll_top: 50.0,
                scroll_left: 0.0,
                viewport_height: 100.0,
                viewport_width: 100.0,
                content_height: 500.0,
                content_width: 100.0,
            },
        );
        w.scroll_to_bottom();
        assert!((w.state.scroll_top - 400.0).abs() < f64::EPSILON);
        w.scroll_to_top();
        assert!((w.state.scroll_top - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn widget_page_up_down() {
        let mut w = ScrollbarWidget::new(
            ScrollbarConfig::default(),
            ScrollState {
                scroll_top: 200.0,
                scroll_left: 0.0,
                viewport_height: 100.0,
                viewport_width: 100.0,
                content_height: 500.0,
                content_width: 100.0,
            },
        );
        w.scroll_page_down();
        assert!((w.state.scroll_top - 300.0).abs() < f64::EPSILON);
        w.scroll_page_up();
        assert!((w.state.scroll_top - 200.0).abs() < f64::EPSILON);

        // Page up from near top clamps to 0
        w.state.scroll_top = 30.0;
        w.scroll_page_up();
        assert!((w.state.scroll_top - 0.0).abs() < f64::EPSILON);
    }
}
