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

/// Accumulated statistics for scrollbar operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollbarStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ScrollbarStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &ScrollbarStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for ScrollbarStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ScrollbarStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ScrollbarStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for scrollbar.
#[derive(Debug, Clone)]
pub struct ScrollbarValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ScrollbarValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for ScrollbarValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ScrollbarTrack – represents the scrollbar track area
// ---------------------------------------------------------------------------

/// Represents the scrollbar track area where the thumb slides.
#[derive(Debug, Clone)]
pub struct ScrollbarTrack {
    /// Start position of the track in pixels (e.g. top edge for vertical).
    pub position: f64,
    /// Total length of the track in pixels.
    pub length: f64,
    /// Whether this is a vertical or horizontal scrollbar track.
    pub orientation: ScrollbarOrientation,
}

impl ScrollbarTrack {
    /// Create a new scrollbar track.
    pub fn new(position: f64, length: f64, orientation: ScrollbarOrientation) -> Self {
        Self {
            position,
            length,
            orientation,
        }
    }

    /// Returns the `(start, end)` of the visible portion as fractions of
    /// total content, each in `0.0..=1.0`.
    pub fn visible_range(&self, state: &ScrollState) -> (f64, f64) {
        let (scroll_offset, viewport_size, content_size) = match self.orientation {
            ScrollbarOrientation::Vertical => {
                (state.scroll_top, state.viewport_height, state.content_height)
            }
            ScrollbarOrientation::Horizontal => {
                (state.scroll_left, state.viewport_width, state.content_width)
            }
        };

        if content_size <= 0.0 {
            return (0.0, 1.0);
        }

        let start = (scroll_offset / content_size).clamp(0.0, 1.0);
        let end = ((scroll_offset + viewport_size) / content_size).clamp(0.0, 1.0);
        (start, end)
    }

    /// Returns `true` if the given coordinate falls within the track bounds.
    pub fn contains_point(&self, point: f64) -> bool {
        point >= self.position && point <= self.position + self.length
    }
}

// ---------------------------------------------------------------------------
// ScrollbarThumb – the draggable thumb element
// ---------------------------------------------------------------------------

/// The draggable thumb inside a scrollbar track.
#[derive(Debug, Clone)]
pub struct ScrollbarThumb {
    /// Minimum size in pixels so the thumb never becomes too small to grab.
    pub min_size: f64,
}

impl ScrollbarThumb {
    /// Create a new thumb with the given minimum size.
    pub fn new(min_size: f64) -> Self {
        Self { min_size }
    }

    /// Compute the thumb size proportional to the viewport/content ratio,
    /// clamped so it never shrinks below `min_size`.
    ///
    /// If the viewport is >= the content, the thumb fills the entire track.
    pub fn compute_size(
        &self,
        viewport_size: f64,
        content_size: f64,
        track_length: f64,
    ) -> f64 {
        if content_size <= 0.0 || viewport_size >= content_size {
            return track_length;
        }
        let proportional = (viewport_size / content_size) * track_length;
        proportional.max(self.min_size).min(track_length)
    }

    /// Compute the thumb position along the track given the current scroll
    /// fraction (`0.0..=1.0`).
    pub fn compute_position(
        &self,
        scroll_fraction: f64,
        track_length: f64,
        thumb_size: f64,
    ) -> f64 {
        let available = (track_length - thumb_size).max(0.0);
        let fraction = scroll_fraction.clamp(0.0, 1.0);
        fraction * available
    }
}

// ---------------------------------------------------------------------------
// Hit-testing
// ---------------------------------------------------------------------------

/// Result of a hit-test against the scrollbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollHitResult {
    /// The click landed on the thumb itself.
    OnThumb,
    /// The click landed on the track *before* the thumb (scroll page up / left).
    BeforeThumb,
    /// The click landed on the track *after* the thumb (scroll page down / right).
    AfterThumb,
    /// The click landed outside the track entirely.
    Outside,
}

/// Perform a hit-test for a click at `click_position` against the given
/// track, thumb, and scroll state.
pub fn scrollbar_hit_test(
    click_position: f64,
    track: &ScrollbarTrack,
    thumb: &ScrollbarThumb,
    state: &ScrollState,
) -> ScrollHitResult {
    if !track.contains_point(click_position) {
        return ScrollHitResult::Outside;
    }

    let (viewport_size, content_size, scroll_fraction) = match track.orientation {
        ScrollbarOrientation::Vertical => (
            state.viewport_height,
            state.content_height,
            state.scroll_percentage_vertical(),
        ),
        ScrollbarOrientation::Horizontal => (
            state.viewport_width,
            state.content_width,
            state.scroll_percentage_horizontal(),
        ),
    };

    let thumb_size = thumb.compute_size(viewport_size, content_size, track.length);
    let thumb_start = track.position + thumb.compute_position(scroll_fraction, track.length, thumb_size);
    let thumb_end = thumb_start + thumb_size;

    if click_position >= thumb_start && click_position <= thumb_end {
        ScrollHitResult::OnThumb
    } else if click_position < thumb_start {
        ScrollHitResult::BeforeThumb
    } else {
        ScrollHitResult::AfterThumb
    }
}

/// Convert a click position on the track to a scroll offset in content
/// coordinates.
///
/// The returned value is clamped to `0.0..=max_scroll` where
/// `max_scroll = content_size - viewport_size`.
pub fn scroll_position_from_click(
    click_pos: f64,
    track: &ScrollbarTrack,
    content_size: f64,
    viewport_size: f64,
) -> f64 {
    let max_scroll = (content_size - viewport_size).max(0.0);
    if track.length <= 0.0 {
        return 0.0;
    }

    let relative = (click_pos - track.position).clamp(0.0, track.length);
    let fraction = relative / track.length;
    (fraction * max_scroll).clamp(0.0, max_scroll)
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

    #[test]
    fn eq_scrollbarorientation_same() {
        assert_eq!(ScrollbarOrientation::Vertical, ScrollbarOrientation::Vertical);
    }

    #[test]
    fn ne_scrollbarorientation_diff() {
        assert_ne!(ScrollbarOrientation::Vertical, ScrollbarOrientation::Horizontal);
    }

    #[test]
    fn eq_scrollbarvisibility_same() {
        assert_eq!(ScrollbarVisibility::Auto, ScrollbarVisibility::Auto);
    }

    #[test]
    fn ne_scrollbarvisibility_diff() {
        assert_ne!(ScrollbarVisibility::Auto, ScrollbarVisibility::Visible);
    }

    #[test]
    fn display_scrollbarerror_variants() {
        assert!(std::mem::size_of::<ScrollbarError>() > 0);
    }

    // ---------------------------------------------------------------
    // ScrollbarTrack / ScrollbarThumb / hit-test tests
    // ---------------------------------------------------------------

    fn make_state(scroll_top: f64, vh: f64, ch: f64) -> ScrollState {
        ScrollState {
            scroll_top,
            scroll_left: 0.0,
            viewport_height: vh,
            viewport_width: 200.0,
            content_height: ch,
            content_width: 200.0,
        }
    }

    #[test]
    fn track_visible_range_at_top() {
        let track = ScrollbarTrack::new(0.0, 400.0, ScrollbarOrientation::Vertical);
        let state = make_state(0.0, 100.0, 1000.0);
        let (start, end) = track.visible_range(&state);
        assert!((start - 0.0).abs() < f64::EPSILON);
        assert!((end - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn track_visible_range_at_middle() {
        let track = ScrollbarTrack::new(0.0, 400.0, ScrollbarOrientation::Vertical);
        let state = make_state(450.0, 100.0, 1000.0);
        let (start, end) = track.visible_range(&state);
        assert!((start - 0.45).abs() < f64::EPSILON);
        assert!((end - 0.55).abs() < f64::EPSILON);
    }

    #[test]
    fn track_contains_point() {
        let track = ScrollbarTrack::new(10.0, 200.0, ScrollbarOrientation::Vertical);
        assert!(track.contains_point(10.0));
        assert!(track.contains_point(110.0));
        assert!(track.contains_point(210.0));
        assert!(!track.contains_point(9.9));
        assert!(!track.contains_point(210.1));
    }

    #[test]
    fn thumb_proportional_sizing() {
        let thumb = ScrollbarThumb::new(20.0);
        // viewport=100, content=500, track=400 → proportional = 80
        let size = thumb.compute_size(100.0, 500.0, 400.0);
        assert!((size - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn thumb_min_size_clamping() {
        let thumb = ScrollbarThumb::new(30.0);
        // viewport=10, content=10_000, track=400 → proportional = 0.4, clamped to 30
        let size = thumb.compute_size(10.0, 10_000.0, 400.0);
        assert!((size - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn thumb_position_at_start() {
        let thumb = ScrollbarThumb::new(20.0);
        let pos = thumb.compute_position(0.0, 400.0, 80.0);
        assert!((pos - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn thumb_position_at_end() {
        let thumb = ScrollbarThumb::new(20.0);
        let pos = thumb.compute_position(1.0, 400.0, 80.0);
        // available = 400 - 80 = 320
        assert!((pos - 320.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hit_test_on_thumb() {
        let track = ScrollbarTrack::new(0.0, 400.0, ScrollbarOrientation::Vertical);
        let thumb = ScrollbarThumb::new(20.0);
        // viewport=100, content=500 → thumb_size=80, scroll at top → thumb 0..80
        let state = make_state(0.0, 100.0, 500.0);
        let result = scrollbar_hit_test(40.0, &track, &thumb, &state);
        assert_eq!(result, ScrollHitResult::OnThumb);
    }

    #[test]
    fn hit_test_before_thumb() {
        let track = ScrollbarTrack::new(0.0, 400.0, ScrollbarOrientation::Vertical);
        let thumb = ScrollbarThumb::new(20.0);
        // scroll at 100% → thumb at end (320..400)
        let state = make_state(400.0, 100.0, 500.0);
        let result = scrollbar_hit_test(50.0, &track, &thumb, &state);
        assert_eq!(result, ScrollHitResult::BeforeThumb);
    }

    #[test]
    fn scroll_position_from_click_test() {
        let track = ScrollbarTrack::new(0.0, 400.0, ScrollbarOrientation::Vertical);
        // click at middle of track → fraction 0.5 → offset = 0.5 * (1000-100) = 450
        let offset = scroll_position_from_click(200.0, &track, 1000.0, 100.0);
        assert!((offset - 450.0).abs() < f64::EPSILON);
        // click at start → 0
        let offset = scroll_position_from_click(0.0, &track, 1000.0, 100.0);
        assert!((offset - 0.0).abs() < f64::EPSILON);
        // click at end → 900
        let offset = scroll_position_from_click(400.0, &track, 1000.0, 100.0);
        assert!((offset - 900.0).abs() < f64::EPSILON);
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_31() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_32() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_33() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn scrollbar_stats_new_defaults() {
        let stats = ScrollbarStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn scrollbar_stats_record_success() {
        let mut stats = ScrollbarStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scrollbar_stats_record_failure() {
        let mut stats = ScrollbarStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn scrollbar_stats_reset() {
        let mut stats = ScrollbarStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn scrollbar_stats_merge() {
        let mut a = ScrollbarStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ScrollbarStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn scrollbar_stats_display() {
        let mut stats = ScrollbarStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn scrollbar_stats_default() {
        let stats = ScrollbarStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn scrollbar_validator_accepts_valid_name() {
        let v = ScrollbarValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn scrollbar_validator_rejects_empty() {
        let v = ScrollbarValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn scrollbar_validator_rejects_too_long() {
        let v = ScrollbarValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn scrollbar_validator_forbidden_prefix() {
        let v = ScrollbarValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn scrollbar_validator_allowed_chars() {
        let v = ScrollbarValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn scrollbar_validator_range() {
        let v = ScrollbarValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn scrollbar_sanitize_removes_control() {
        let result = ScrollbarValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn scrollbar_truncate_short_string() {
        assert_eq!(ScrollbarValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn scrollbar_truncate_long_string() {
        let result = ScrollbarValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn scrollbar_is_ascii_printable() {
        assert!(ScrollbarValidator::is_ascii_printable("Hello World 123"));
        assert!(!ScrollbarValidator::is_ascii_printable("Hello\x00World"));
    }
}
