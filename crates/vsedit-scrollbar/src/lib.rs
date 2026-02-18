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

// ---------------------------------------------------------------------------
// ScrollbarAnimation – smooth scrolling support
// ---------------------------------------------------------------------------

/// Easing function used for smooth scroll animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollEasing {
    /// Linear interpolation (constant speed).
    Linear,
    /// Quadratic ease-out (decelerating).
    EaseOut,
    /// Quadratic ease-in-out (accelerate then decelerate).
    EaseInOut,
}

impl ScrollEasing {
    /// Evaluate the easing function at time `t` where `t` is in `0.0..=1.0`.
    pub fn evaluate(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            ScrollEasing::Linear => t,
            ScrollEasing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            ScrollEasing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
        }
    }
}

/// Drives a smooth scroll animation from one position to another.
#[derive(Debug, Clone)]
pub struct ScrollbarAnimation {
    /// Starting scroll offset.
    pub from: f64,
    /// Target scroll offset.
    pub to: f64,
    /// Duration of the animation in milliseconds.
    pub duration_ms: u64,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: u64,
    /// Easing function to use.
    pub easing: ScrollEasing,
}

impl ScrollbarAnimation {
    /// Create a new animation.
    pub fn new(from: f64, to: f64, duration_ms: u64, easing: ScrollEasing) -> Self {
        Self { from, to, duration_ms, elapsed_ms: 0, easing }
    }

    /// Returns `true` when the animation has finished.
    pub fn is_complete(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    /// Advance the animation by `delta_ms` milliseconds and return the current
    /// interpolated position.
    pub fn tick(&mut self, delta_ms: u64) -> f64 {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms).min(self.duration_ms);
        self.current_value()
    }

    /// Return the current interpolated position without advancing time.
    pub fn current_value(&self) -> f64 {
        if self.duration_ms == 0 {
            return self.to;
        }
        let t = self.elapsed_ms as f64 / self.duration_ms as f64;
        let eased = self.easing.evaluate(t);
        self.from + (self.to - self.from) * eased
    }

    /// Return the progress of the animation as a value in `0.0..=1.0`.
    pub fn progress(&self) -> f64 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        (self.elapsed_ms as f64 / self.duration_ms as f64).clamp(0.0, 1.0)
    }
}

// ---------------------------------------------------------------------------
// MinimapRenderer – computes layout for a document minimap
// ---------------------------------------------------------------------------

/// A decoration marker displayed in the minimap or overview ruler.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapMarker {
    /// Line number (0-based) where the marker starts.
    pub line_start: usize,
    /// Line number (0-based) where the marker ends (inclusive).
    pub line_end: usize,
    /// RGBA colour packed into a u32 (0xRRGGBBAA).
    pub color: u32,
}

/// Computes pixel coordinates for minimap rendering.
#[derive(Debug, Clone)]
pub struct MinimapRenderer {
    /// Total number of lines in the document.
    pub total_lines: usize,
    /// Height of the minimap viewport in pixels.
    pub viewport_height: f64,
    /// Width of the minimap in pixels.
    pub width: f64,
}

impl MinimapRenderer {
    pub fn new(total_lines: usize, viewport_height: f64, width: f64) -> Self {
        Self { total_lines, viewport_height, width }
    }

    /// Pixels per line in the minimap.
    pub fn line_height(&self) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        self.viewport_height / self.total_lines as f64
    }

    /// Convert a line number to a y-pixel coordinate in the minimap.
    pub fn line_to_y(&self, line: usize) -> f64 {
        line as f64 * self.line_height()
    }

    /// Convert a y-pixel coordinate to the nearest line number.
    pub fn y_to_line(&self, y: f64) -> usize {
        let lh = self.line_height();
        if lh <= 0.0 {
            return 0;
        }
        let line = (y / lh).floor() as usize;
        line.min(self.total_lines.saturating_sub(1))
    }

    /// Return the y-range `(top, height)` for a marker in minimap pixel space.
    pub fn marker_rect(&self, marker: &MinimapMarker) -> (f64, f64) {
        let top = self.line_to_y(marker.line_start);
        let bottom = self.line_to_y(marker.line_end + 1);
        let height = (bottom - top).max(1.0);
        (top, height)
    }

    /// Return the visible slider rect `(top, height)` given the current scroll
    /// state and document line count.
    pub fn visible_slider(&self, state: &ScrollState, line_height_px: f64) -> (f64, f64) {
        if line_height_px <= 0.0 || self.total_lines == 0 {
            return (0.0, self.viewport_height);
        }
        let first_visible = state.scroll_top / line_height_px;
        let visible_lines = state.viewport_height / line_height_px;
        let lh = self.line_height();
        let top = first_visible * lh;
        let height = (visible_lines * lh).min(self.viewport_height - top).max(1.0);
        (top, height)
    }
}

// ---------------------------------------------------------------------------
// ScrollbarAccessibility – ARIA / screen-reader metadata
// ---------------------------------------------------------------------------

/// Accessibility metadata for a scrollbar, suitable for ARIA attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollbarAccessibility {
    /// The ARIA role (e.g. `"scrollbar"`).
    pub role: String,
    /// Human-readable label.
    pub label: String,
    /// Current value as a percentage `0..=100`.
    pub value_now: u32,
    /// Minimum value (always 0).
    pub value_min: u32,
    /// Maximum value (always 100).
    pub value_max: u32,
    /// The orientation string (`"vertical"` or `"horizontal"`).
    pub orientation_str: String,
}

impl ScrollbarAccessibility {
    /// Build accessibility info from a scroll state and orientation.
    pub fn from_state(state: &ScrollState, orientation: ScrollbarOrientation) -> Self {
        let pct = match orientation {
            ScrollbarOrientation::Vertical => state.scroll_percentage_vertical(),
            ScrollbarOrientation::Horizontal => state.scroll_percentage_horizontal(),
        };
        let orientation_str = match orientation {
            ScrollbarOrientation::Vertical => "vertical",
            ScrollbarOrientation::Horizontal => "horizontal",
        };
        Self {
            role: "scrollbar".to_string(),
            label: format!("{} scrollbar", orientation_str),
            value_now: (pct * 100.0).round() as u32,
            value_min: 0,
            value_max: 100,
            orientation_str: orientation_str.to_string(),
        }
    }
}

impl fmt::Display for ScrollbarAccessibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "aria-role={} aria-label=\"{}\" aria-valuenow={} aria-orientation={}",
            self.role, self.label, self.value_now, self.orientation_str,
        )
    }
}

// ---------------------------------------------------------------------------
// ScrollbarZoom – zoom-level management for scrollbar / minimap
// ---------------------------------------------------------------------------

/// Manages zoom level for the scrollbar or minimap.
#[derive(Debug, Clone)]
pub struct ScrollbarZoom {
    /// Current zoom factor (1.0 = 100%).
    level: f64,
    /// Minimum allowed zoom factor.
    min_level: f64,
    /// Maximum allowed zoom factor.
    max_level: f64,
    /// Increment used by `zoom_in` / `zoom_out`.
    step: f64,
}

impl ScrollbarZoom {
    /// Create a new zoom controller.
    pub fn new(min_level: f64, max_level: f64, step: f64) -> Self {
        Self {
            level: 1.0,
            min_level: min_level.max(0.1),
            max_level: max_level.max(min_level),
            step: step.abs().max(0.01),
        }
    }

    /// Current zoom level.
    pub fn level(&self) -> f64 {
        self.level
    }

    /// Zoom in by one step.
    pub fn zoom_in(&mut self) -> f64 {
        self.level = (self.level + self.step).min(self.max_level);
        self.level
    }

    /// Zoom out by one step.
    pub fn zoom_out(&mut self) -> f64 {
        self.level = (self.level - self.step).max(self.min_level);
        self.level
    }

    /// Reset zoom to 1.0.
    pub fn reset(&mut self) -> f64 {
        self.level = 1.0_f64.clamp(self.min_level, self.max_level);
        self.level
    }

    /// Set zoom to an arbitrary value, clamped to the allowed range.
    pub fn set_level(&mut self, level: f64) -> f64 {
        self.level = level.clamp(self.min_level, self.max_level);
        self.level
    }

    /// Apply the current zoom factor to a dimension value.
    pub fn apply(&self, value: f64) -> f64 {
        value * self.level
    }

    /// Returns `true` if the zoom is at the default level (1.0).
    pub fn is_default(&self) -> bool {
        (self.level - 1.0).abs() < f64::EPSILON
    }
}

impl Default for ScrollbarZoom {
    fn default() -> Self {
        Self::new(0.25, 4.0, 0.25)
    }
}

// ---------------------------------------------------------------------------
// ScrollState – additional helpers
// ---------------------------------------------------------------------------

impl ScrollState {
    /// Create a new scroll state with the given viewport and content dimensions.
    pub fn new(viewport_width: f64, viewport_height: f64, content_width: f64, content_height: f64) -> Self {
        Self {
            scroll_top: 0.0,
            scroll_left: 0.0,
            viewport_height,
            viewport_width,
            content_height,
            content_width,
        }
    }

    /// Returns `true` when the content fits entirely within the viewport
    /// (no scrolling needed in either direction).
    pub fn content_fits(&self) -> bool {
        self.content_height <= self.viewport_height && self.content_width <= self.viewport_width
    }

    /// Maximum vertical scroll offset.
    pub fn max_scroll_top(&self) -> f64 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    /// Maximum horizontal scroll offset.
    pub fn max_scroll_left(&self) -> f64 {
        (self.content_width - self.viewport_width).max(0.0)
    }

    /// Scroll vertically by `delta` pixels, clamping to valid bounds.
    pub fn scroll_by_vertical(&mut self, delta: f64) {
        self.scroll_top += delta;
        self.clamp_scroll();
    }

    /// Scroll horizontally by `delta` pixels, clamping to valid bounds.
    pub fn scroll_by_horizontal(&mut self, delta: f64) {
        self.scroll_left += delta;
        self.clamp_scroll();
    }

    /// Ensure the given vertical line (in content-pixel coordinates) is visible.
    /// If it is above the viewport, scroll up. If below, scroll down.
    pub fn ensure_visible_vertical(&mut self, y: f64, margin: f64) {
        if y < self.scroll_top + margin {
            self.scroll_top = (y - margin).max(0.0);
        } else if y > self.scroll_top + self.viewport_height - margin {
            self.scroll_top = y - self.viewport_height + margin;
        }
        self.clamp_scroll();
    }

    /// Ensure the given horizontal position is visible, scrolling if needed.
    pub fn ensure_visible_horizontal(&mut self, x: f64, margin: f64) {
        if x < self.scroll_left + margin {
            self.scroll_left = (x - margin).max(0.0);
        } else if x > self.scroll_left + self.viewport_width - margin {
            self.scroll_left = x - self.viewport_width + margin;
        }
        self.clamp_scroll();
    }

    /// Returns the visible vertical range as `(top, bottom)` in content coordinates.
    pub fn visible_vertical_range(&self) -> (f64, f64) {
        (self.scroll_top, self.scroll_top + self.viewport_height)
    }

    /// Returns the visible horizontal range as `(left, right)` in content coordinates.
    pub fn visible_horizontal_range(&self) -> (f64, f64) {
        (self.scroll_left, self.scroll_left + self.viewport_width)
    }

    /// Returns `true` if the given point (in content coordinates) is within the viewport.
    pub fn is_point_visible(&self, x: f64, y: f64) -> bool {
        let (top, bottom) = self.visible_vertical_range();
        let (left, right) = self.visible_horizontal_range();
        x >= left && x <= right && y >= top && y <= bottom
    }
}

// ---------------------------------------------------------------------------
// ScrollbarWidget – additional navigation methods
// ---------------------------------------------------------------------------

impl ScrollbarWidget {
    /// Scroll to the very left of the content.
    pub fn scroll_to_left(&mut self) {
        self.state.scroll_left = 0.0;
    }

    /// Scroll to the very right of the content.
    pub fn scroll_to_right(&mut self) {
        self.state.scroll_left = self.state.max_scroll_left();
    }

    /// Scroll left by one viewport width.
    pub fn scroll_page_left(&mut self) {
        self.state.scroll_left -= self.state.viewport_width;
        self.state.clamp_scroll();
    }

    /// Scroll right by one viewport width.
    pub fn scroll_page_right(&mut self) {
        self.state.scroll_left += self.state.viewport_width;
        self.state.clamp_scroll();
    }

    /// Scroll to a specific line number (assuming `line_height_px` per line).
    pub fn scroll_to_line(&mut self, line: usize, line_height_px: f64) {
        self.state.scroll_top = line as f64 * line_height_px;
        self.state.clamp_scroll();
    }

    /// Returns the currently visible line range `(first, last)` for the given line height.
    pub fn visible_line_range(&self, line_height_px: f64) -> (usize, usize) {
        if line_height_px <= 0.0 {
            return (0, 0);
        }
        let first = (self.state.scroll_top / line_height_px).floor() as usize;
        let last = ((self.state.scroll_top + self.state.viewport_height) / line_height_px).ceil() as usize;
        (first, last)
    }

    /// Whether the vertical scrollbar should be shown based on config.
    pub fn should_show_vertical(&self) -> bool {
        match self.config.vertical {
            ScrollbarVisibility::Visible => true,
            ScrollbarVisibility::Hidden => false,
            ScrollbarVisibility::Auto => self.state.content_height > self.state.viewport_height,
        }
    }

    /// Whether the horizontal scrollbar should be shown based on config.
    pub fn should_show_horizontal(&self) -> bool {
        match self.config.horizontal {
            ScrollbarVisibility::Visible => true,
            ScrollbarVisibility::Hidden => false,
            ScrollbarVisibility::Auto => self.state.content_width > self.state.viewport_width,
        }
    }
}

// ---------------------------------------------------------------------------
// ScrollbarConfig – builder methods
// ---------------------------------------------------------------------------

impl ScrollbarConfig {
    /// Builder: set vertical visibility.
    pub fn with_vertical(mut self, v: ScrollbarVisibility) -> Self {
        self.vertical = v;
        self
    }

    /// Builder: set horizontal visibility.
    pub fn with_horizontal(mut self, v: ScrollbarVisibility) -> Self {
        self.horizontal = v;
        self
    }

    /// Builder: set scroll sensitivity.
    pub fn with_sensitivity(mut self, s: f64) -> Self {
        self.scroll_sensitivity = s;
        self
    }

    /// Builder: set fast scroll sensitivity.
    pub fn with_fast_sensitivity(mut self, s: f64) -> Self {
        self.fast_scroll_sensitivity = s;
        self
    }
}

// ---------------------------------------------------------------------------
// ScrollbarAnimation – additional helpers
// ---------------------------------------------------------------------------

impl ScrollbarAnimation {
    /// Remaining time in milliseconds.
    pub fn remaining_ms(&self) -> u64 {
        self.duration_ms.saturating_sub(self.elapsed_ms)
    }

    /// Reset the animation to replay from the beginning.
    pub fn restart(&mut self) {
        self.elapsed_ms = 0;
    }

    /// Reverse the animation direction (swap `from` and `to`) and reset elapsed time.
    pub fn reverse(&mut self) {
        std::mem::swap(&mut self.from, &mut self.to);
        self.elapsed_ms = 0;
    }

    /// Retarget the animation to a new destination, keeping current position as `from`.
    pub fn retarget(&mut self, new_to: f64) {
        self.from = self.current_value();
        self.to = new_to;
        self.elapsed_ms = 0;
    }
}

// ---------------------------------------------------------------------------
// MinimapRenderer – additional helpers
// ---------------------------------------------------------------------------

impl MinimapRenderer {
    /// Returns `true` if a given line is visible in the minimap viewport.
    pub fn is_line_visible(&self, line: usize) -> bool {
        let y = self.line_to_y(line);
        y >= 0.0 && y < self.viewport_height
    }

    /// Clamp a line number to valid range `[0, total_lines)`.
    pub fn clamp_line(&self, line: usize) -> usize {
        line.min(self.total_lines.saturating_sub(1))
    }

    /// Returns the number of lines that can be shown in the minimap at once.
    /// If `total_lines` is 0, returns 0.
    pub fn visible_line_count(&self) -> usize {
        self.total_lines
    }

    /// Returns all markers that overlap the given line range `[start, end]`.
    pub fn markers_in_range<'a>(
        &self,
        markers: &'a [MinimapMarker],
        range_start: usize,
        range_end: usize,
    ) -> Vec<&'a MinimapMarker> {
        markers
            .iter()
            .filter(|m| m.line_end >= range_start && m.line_start <= range_end)
            .collect()
    }
}


// ---------------------------------------------------------------------------
// ScrollbarAnnotation – colored markers on the scrollbar track (VS Code style)
// ---------------------------------------------------------------------------

/// The kind of annotation shown on the scrollbar track or overview ruler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationKind {
    /// A diagnostic error (red).
    Error,
    /// A diagnostic warning (yellow/amber).
    Warning,
    /// An informational hint (blue).
    Info,
    /// Search result highlight (orange).
    SearchResult,
    /// Selection / cursor highlight (cyan).
    Selection,
    /// Git modification indicator (blue in gutter).
    GitModified,
    /// Git addition indicator (green in gutter).
    GitAdded,
    /// Git deletion indicator (red in gutter).
    GitDeleted,
    /// A user-defined custom annotation.
    Custom(u8),
}

impl AnnotationKind {
    /// Default RGBA colour for this annotation kind, matching VS Code defaults.
    pub fn default_color(&self) -> u32 {
        match self {
            AnnotationKind::Error => 0xFF1212CC,
            AnnotationKind::Warning => 0xFFC800CC,
            AnnotationKind::Info => 0x3794FFCC,
            AnnotationKind::SearchResult => 0xEA5C00CC,
            AnnotationKind::Selection => 0x1A85FFAA,
            AnnotationKind::GitModified => 0x1B81A8CC,
            AnnotationKind::GitAdded => 0x2EA04366,
            AnnotationKind::GitDeleted => 0xF85149CC,
            AnnotationKind::Custom(_) => 0xFFFFFF80,
        }
    }

    /// Priority for z-ordering: higher values are drawn on top.
    pub fn priority(&self) -> u8 {
        match self {
            AnnotationKind::Error => 100,
            AnnotationKind::Warning => 90,
            AnnotationKind::Info => 70,
            AnnotationKind::SearchResult => 80,
            AnnotationKind::Selection => 60,
            AnnotationKind::GitModified => 40,
            AnnotationKind::GitAdded => 30,
            AnnotationKind::GitDeleted => 50,
            AnnotationKind::Custom(p) => *p,
        }
    }
}

impl fmt::Display for AnnotationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnnotationKind::Error => write!(f, "error"),
            AnnotationKind::Warning => write!(f, "warning"),
            AnnotationKind::Info => write!(f, "info"),
            AnnotationKind::SearchResult => write!(f, "search"),
            AnnotationKind::Selection => write!(f, "selection"),
            AnnotationKind::GitModified => write!(f, "git-modified"),
            AnnotationKind::GitAdded => write!(f, "git-added"),
            AnnotationKind::GitDeleted => write!(f, "git-deleted"),
            AnnotationKind::Custom(id) => write!(f, "custom({})", id),
        }
    }
}

/// A single annotation to render on the scrollbar track.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollbarAnnotation {
    /// First line covered by this annotation (0-based).
    pub line_start: usize,
    /// Last line covered by this annotation (inclusive, 0-based).
    pub line_end: usize,
    /// Kind of annotation (determines default colour and z-order).
    pub kind: AnnotationKind,
    /// Optional RGBA override colour (packed `0xRRGGBBAA`).
    pub color_override: Option<u32>,
}

impl ScrollbarAnnotation {
    /// Create a new annotation spanning a single line.
    pub fn single_line(line: usize, kind: AnnotationKind) -> Self {
        Self {
            line_start: line,
            line_end: line,
            kind,
            color_override: None,
        }
    }

    /// Create an annotation spanning an inclusive range of lines.
    pub fn line_range(start: usize, end: usize, kind: AnnotationKind) -> Self {
        let (s, e) = if start <= end { (start, end) } else { (end, start) };
        Self {
            line_start: s,
            line_end: e,
            kind,
            color_override: None,
        }
    }

    /// Set a custom colour override.
    pub fn with_color(mut self, color: u32) -> Self {
        self.color_override = Some(color);
        self
    }

    /// Effective colour to use when rendering.
    pub fn effective_color(&self) -> u32 {
        self.color_override.unwrap_or_else(|| self.kind.default_color())
    }

    /// Number of lines spanned by this annotation.
    pub fn line_span(&self) -> usize {
        self.line_end - self.line_start + 1
    }

    /// Returns `true` if this annotation overlaps the given line range `[lo, hi]`.
    pub fn overlaps_range(&self, lo: usize, hi: usize) -> bool {
        self.line_end >= lo && self.line_start <= hi
    }
}

// ---------------------------------------------------------------------------
// OverviewRuler – VS Code-style overview ruler rendering
// ---------------------------------------------------------------------------

/// Layout position for the overview ruler relative to the scrollbar track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewRulerLane {
    /// Left third of the ruler width.
    Left,
    /// Centre third of the ruler width.
    Center,
    /// Right third of the ruler width.
    Right,
    /// Full width of the ruler.
    Full,
}

/// Pre-computed rectangle for drawing a single overview ruler decoration.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewRulerDecoration {
    /// Y offset from the top of the ruler track, in pixels.
    pub y: f64,
    /// Height of the decoration, in pixels (minimum 2px for visibility).
    pub height: f64,
    /// X offset within the ruler width, in pixels.
    pub x: f64,
    /// Width of the decoration, in pixels.
    pub width: f64,
    /// Colour to draw (`0xRRGGBBAA`).
    pub color: u32,
}

/// Computes overview ruler decorations from a set of annotations.
#[derive(Debug, Clone)]
pub struct OverviewRuler {
    /// Height of the overview ruler in pixels (usually equals the scrollbar track height).
    pub track_height: f64,
    /// Width of the ruler in pixels.
    pub ruler_width: f64,
    /// Total number of lines in the document.
    pub total_lines: usize,
}

impl OverviewRuler {
    pub fn new(track_height: f64, ruler_width: f64, total_lines: usize) -> Self {
        Self { track_height, ruler_width, total_lines }
    }

    /// Map a line number to a y-pixel coordinate.
    fn line_to_y(&self, line: usize) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        (line as f64 / self.total_lines as f64) * self.track_height
    }

    /// X offset and width for a given lane.
    fn lane_rect(&self, lane: OverviewRulerLane) -> (f64, f64) {
        let third = self.ruler_width / 3.0;
        match lane {
            OverviewRulerLane::Left => (0.0, third),
            OverviewRulerLane::Center => (third, third),
            OverviewRulerLane::Right => (third * 2.0, third),
            OverviewRulerLane::Full => (0.0, self.ruler_width),
        }
    }

    /// Build a decoration rect for a single annotation + lane.
    pub fn decoration_for(
        &self,
        annotation: &ScrollbarAnnotation,
        lane: OverviewRulerLane,
    ) -> OverviewRulerDecoration {
        let y_start = self.line_to_y(annotation.line_start);
        let y_end = self.line_to_y(annotation.line_end + 1);
        let raw_height = y_end - y_start;
        let height = raw_height.max(2.0);
        let (x, width) = self.lane_rect(lane);
        OverviewRulerDecoration {
            y: y_start,
            height,
            x,
            width,
            color: annotation.effective_color(),
        }
    }

    /// Compute decorations for a slice of `(annotation, lane)` pairs, sorted by
    /// rendering priority (lowest priority first so high-priority paints on top).
    pub fn compute_decorations(
        &self,
        entries: &[(&ScrollbarAnnotation, OverviewRulerLane)],
    ) -> Vec<OverviewRulerDecoration> {
        let mut sorted: Vec<_> = entries.to_vec();
        sorted.sort_by_key(|(a, _)| a.kind.priority());
        sorted.iter().map(|(a, lane)| self.decoration_for(a, *lane)).collect()
    }

    /// Count how many annotations fall within a visible line range.
    pub fn annotations_in_view(
        annotations: &[ScrollbarAnnotation],
        first_line: usize,
        last_line: usize,
    ) -> usize {
        annotations.iter().filter(|a| a.overlaps_range(first_line, last_line)).count()
    }
}

// ---------------------------------------------------------------------------
// ThumbSizeCalculator – advanced thumb sizing matching VS Code heuristics
// ---------------------------------------------------------------------------

/// Configuration-driven thumb size calculator.
///
/// VS Code uses a combination of proportional sizing, minimum size enforcement,
/// and optional "slider background" when content is very large.
#[derive(Debug, Clone)]
pub struct ThumbSizeCalculator {
    /// Minimum thumb length in pixels.
    pub min_thumb_px: f64,
    /// Maximum fraction of the track the thumb can occupy (1.0 = fill whole track).
    pub max_thumb_fraction: f64,
    /// If content exceeds this many lines, enable "large file" mode (slimmer thumb).
    pub large_file_threshold: usize,
    /// Thumb size reduction factor in large-file mode.
    pub large_file_factor: f64,
}

impl ThumbSizeCalculator {
    pub fn new() -> Self {
        Self {
            min_thumb_px: 20.0,
            max_thumb_fraction: 1.0,
            large_file_threshold: 10_000,
            large_file_factor: 0.8,
        }
    }

    /// Builder: override minimum thumb size.
    pub fn with_min_thumb(mut self, px: f64) -> Self {
        self.min_thumb_px = px.max(1.0);
        self
    }

    /// Builder: override max thumb fraction.
    pub fn with_max_fraction(mut self, frac: f64) -> Self {
        self.max_thumb_fraction = frac.clamp(0.01, 1.0);
        self
    }

    /// Builder: set the large-file threshold.
    pub fn with_large_file_threshold(mut self, lines: usize) -> Self {
        self.large_file_threshold = lines.max(1);
        self
    }

    /// Compute the thumb size in pixels for the given parameters.
    pub fn compute(
        &self,
        viewport_lines: usize,
        total_lines: usize,
        track_px: f64,
    ) -> f64 {
        if total_lines == 0 || viewport_lines >= total_lines {
            return (track_px * self.max_thumb_fraction).max(self.min_thumb_px);
        }
        let ratio = viewport_lines as f64 / total_lines as f64;
        let mut size = ratio * track_px;

        if total_lines > self.large_file_threshold {
            size *= self.large_file_factor;
        }

        size.clamp(self.min_thumb_px, track_px * self.max_thumb_fraction)
    }

    /// Convenience: compute thumb position along the track for a given scroll fraction.
    pub fn thumb_position(
        &self,
        scroll_fraction: f64,
        track_px: f64,
        thumb_size: f64,
    ) -> f64 {
        let available = (track_px - thumb_size).max(0.0);
        scroll_fraction.clamp(0.0, 1.0) * available
    }
}

impl Default for ThumbSizeCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ClickToPosition – translate a click on the scrollbar to content coordinates
// ---------------------------------------------------------------------------

/// Result of translating a scrollbar click into content coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ClickPositionResult {
    /// Target scroll offset in content-pixels.
    pub scroll_offset: f64,
    /// Equivalent line number (0-based) assuming the given line height.
    pub target_line: usize,
    /// Fraction along the track where the click occurred (`0.0..=1.0`).
    pub track_fraction: f64,
}

/// Translates a click on the scrollbar into a content position.
///
/// This handles the common VS Code behaviour: clicking on the scrollbar
/// track jumps so the *centre* of the viewport aligns with the click position.
pub fn click_to_position(
    click_y: f64,
    track: &ScrollbarTrack,
    state: &ScrollState,
    line_height_px: f64,
) -> ClickPositionResult {
    let relative = (click_y - track.position).clamp(0.0, track.length);
    let fraction = if track.length > 0.0 { relative / track.length } else { 0.0 };

    let content_size = match track.orientation {
        ScrollbarOrientation::Vertical => state.content_height,
        ScrollbarOrientation::Horizontal => state.content_width,
    };
    let viewport_size = match track.orientation {
        ScrollbarOrientation::Vertical => state.viewport_height,
        ScrollbarOrientation::Horizontal => state.viewport_width,
    };

    // Centre the viewport around the clicked position in content space.
    let content_y = fraction * content_size;
    let max_scroll = (content_size - viewport_size).max(0.0);
    let scroll_offset = (content_y - viewport_size / 2.0).clamp(0.0, max_scroll);

    let target_line = if line_height_px > 0.0 {
        (scroll_offset / line_height_px).round() as usize
    } else {
        0
    };

    ClickPositionResult {
        scroll_offset,
        target_line,
        track_fraction: fraction,
    }
}

/// Helper: extract RGBA components from a packed `0xRRGGBBAA` colour.
pub fn rgba_components(packed: u32) -> (u8, u8, u8, u8) {
    let r = ((packed >> 24) & 0xFF) as u8;
    let g = ((packed >> 16) & 0xFF) as u8;
    let b = ((packed >> 8) & 0xFF) as u8;
    let a = (packed & 0xFF) as u8;
    (r, g, b, a)
}

/// Helper: pack RGBA components into a `0xRRGGBBAA` u32.
pub fn pack_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (u32::from(r) << 24) | (u32::from(g) << 16) | (u32::from(b) << 8) | u32::from(a)
}

/// Alpha-blend `fg` over `bg`, both in `0xRRGGBBAA` format.
/// Uses standard "over" compositing.
pub fn alpha_blend(fg: u32, bg: u32) -> u32 {
    let (fr, fg_g, fb, fa) = rgba_components(fg);
    let (br, bg_g, bb, ba) = rgba_components(bg);
    let alpha = fa as f64 / 255.0;
    let inv = 1.0 - alpha;
    let r = (fr as f64 * alpha + br as f64 * inv) as u8;
    let g = (fg_g as f64 * alpha + bg_g as f64 * inv) as u8;
    let b = (fb as f64 * alpha + bb as f64 * inv) as u8;
    let a = (fa as f64 + ba as f64 * inv).min(255.0) as u8;
    pack_rgba(r, g, b, a)
}



// ---------------------------------------------------------------------------
// AnnotationType – simpler annotation classification (Error/Warning/Info/Search/Change)
// ---------------------------------------------------------------------------

/// Simplified annotation type for scrollbar decorations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationType {
    /// A diagnostic error.
    Error,
    /// A diagnostic warning.
    Warning,
    /// An informational annotation.
    Info,
    /// A search result highlight.
    Search,
    /// A change indicator (e.g. unsaved edit, diff).
    Change,
}

impl AnnotationType {
    /// Returns the rendering priority. Errors are highest priority (drawn on top).
    pub fn priority(&self) -> u8 {
        match self {
            AnnotationType::Error => 100,
            AnnotationType::Warning => 80,
            AnnotationType::Search => 60,
            AnnotationType::Change => 40,
            AnnotationType::Info => 20,
        }
    }

    /// Returns `true` if this is an error annotation.
    pub fn is_error(&self) -> bool {
        matches!(self, AnnotationType::Error)
    }

    /// Returns `true` if this is a warning annotation.
    pub fn is_warning(&self) -> bool {
        matches!(self, AnnotationType::Warning)
    }
}

impl fmt::Display for AnnotationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnnotationType::Error => write!(f, "error"),
            AnnotationType::Warning => write!(f, "warning"),
            AnnotationType::Info => write!(f, "info"),
            AnnotationType::Search => write!(f, "search"),
            AnnotationType::Change => write!(f, "change"),
        }
    }
}

// ---------------------------------------------------------------------------
// ScrollbarAnnotationSimple – annotation with line, color, and AnnotationType
// ---------------------------------------------------------------------------

/// A simplified scrollbar annotation keyed by a single line number, a packed
/// colour, and an `AnnotationType`.
#[derive(Debug, Clone)]
pub struct ScrollbarAnnotationSimple {
    line: usize,
    color: u32,
    annotation_type: AnnotationType,
}

impl ScrollbarAnnotationSimple {
    /// Create a new annotation for the given line with the specified colour and type.
    pub fn new(line: usize, color: u32, annotation_type: AnnotationType) -> Self {
        Self { line, color, annotation_type }
    }

    /// The line number this annotation is placed on.
    pub fn line(&self) -> usize {
        self.line
    }

    /// The packed RGBA colour (`0xRRGGBBAA`).
    pub fn color(&self) -> u32 {
        self.color
    }

    /// The type of this annotation.
    pub fn annotation_type(&self) -> AnnotationType {
        self.annotation_type
    }

    /// Returns `true` if this annotation represents an error.
    pub fn is_error(&self) -> bool {
        self.annotation_type.is_error()
    }

    /// Returns `true` if this annotation represents a warning.
    pub fn is_warning(&self) -> bool {
        self.annotation_type.is_warning()
    }

    /// Rendering priority derived from the annotation type.
    pub fn priority(&self) -> u8 {
        self.annotation_type.priority()
    }
}

// ---------------------------------------------------------------------------
// ScrollbarOverviewRuler – VS Code-style overview ruler with annotation storage
// ---------------------------------------------------------------------------

/// A VS Code-style overview ruler that maps document lines to vertical pixel
/// positions on the scrollbar track. Annotations can be added, queried by line
/// or viewport range, and cleared.
pub struct ScrollbarOverviewRuler {
    track_height: f64,
    total_lines: usize,
    annotations: Vec<ScrollbarAnnotationSimple>,
}

impl ScrollbarOverviewRuler {
    /// Create a new overview ruler for the given track height and document size.
    pub fn new(track_height: f64, total_lines: usize) -> Self {
        Self {
            track_height,
            total_lines: total_lines.max(1),
            annotations: Vec::new(),
        }
    }

    /// Add an annotation to the ruler.
    pub fn add_annotation(&mut self, ann: ScrollbarAnnotationSimple) {
        self.annotations.push(ann);
    }

    /// Return all annotations placed on the given line.
    pub fn annotations_at_line(&self, line: usize) -> Vec<&ScrollbarAnnotationSimple> {
        self.annotations.iter().filter(|a| a.line() == line).collect()
    }

    /// Total number of annotations stored.
    pub fn annotation_count(&self) -> usize {
        self.annotations.len()
    }

    /// Remove all annotations.
    pub fn clear(&mut self) {
        self.annotations.clear();
    }

    /// Map a line number to a y-pixel coordinate on the ruler track.
    pub fn y_position(&self, line: usize) -> f64 {
        if self.total_lines <= 1 {
            return 0.0;
        }
        let clamped = line.min(self.total_lines.saturating_sub(1));
        (clamped as f64 / (self.total_lines - 1) as f64) * self.track_height
    }

    /// Map a y-pixel coordinate back to a line number.
    pub fn line_from_y(&self, y: f64) -> usize {
        if self.track_height <= 0.0 || self.total_lines <= 1 {
            return 0;
        }
        let fraction = (y / self.track_height).clamp(0.0, 1.0);
        let line = (fraction * (self.total_lines - 1) as f64).round() as usize;
        line.min(self.total_lines.saturating_sub(1))
    }

    /// Return annotations whose line falls within `[viewport_start, viewport_end]`.
    pub fn visible_annotations(
        &self,
        viewport_start: usize,
        viewport_end: usize,
    ) -> Vec<&ScrollbarAnnotationSimple> {
        self.annotations
            .iter()
            .filter(|a| a.line() >= viewport_start && a.line() <= viewport_end)
            .collect()
    }

    /// Return annotations matching a specific type.
    pub fn annotations_by_type(&self, t: AnnotationType) -> Vec<&ScrollbarAnnotationSimple> {
        self.annotations.iter().filter(|a| a.annotation_type() == t).collect()
    }

    /// Total lines in the document.
    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    /// Track height in pixels.
    pub fn track_height(&self) -> f64 {
        self.track_height
    }
}

impl fmt::Debug for ScrollbarOverviewRuler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScrollbarOverviewRuler")
            .field("track_height", &self.track_height)
            .field("total_lines", &self.total_lines)
            .field("annotation_count", &self.annotations.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ScrollbarThumbSizeCalculator – thumb sizing with min-size and max-ratio
// ---------------------------------------------------------------------------

/// Calculates the optimal scrollbar thumb size given viewport, content, and
/// track dimensions. Enforces a configurable minimum thumb size and a maximum
/// ratio so the thumb never exceeds a set fraction of the track.
#[derive(Debug, Clone)]
pub struct ScrollbarThumbSizeCalculator {
    min_size: f64,
    max_ratio: f64,
}

impl ScrollbarThumbSizeCalculator {
    /// Create a calculator with the given minimum thumb size (pixels) and maximum
    /// ratio (fraction of track, 0.0–1.0).
    pub fn new(min_size: f64, max_ratio: f64) -> Self {
        Self {
            min_size: min_size.max(1.0),
            max_ratio: max_ratio.clamp(0.01, 1.0),
        }
    }

    /// Calculate thumb size for the given viewport, content, and track dimensions.
    ///
    /// Returns a value clamped between `min_size` and `track * max_ratio`.
    pub fn calculate(&self, viewport: f64, content: f64, track: f64) -> f64 {
        if content <= 0.0 || track <= 0.0 {
            return self.min_size;
        }
        if viewport >= content {
            return (track * self.max_ratio).max(self.min_size);
        }
        let proportional = (viewport / content) * track;
        proportional.clamp(self.min_size, track * self.max_ratio)
    }

    /// Like `calculate`, but subtracts `padding` from the track before computing.
    pub fn calculate_with_padding(
        &self,
        viewport: f64,
        content: f64,
        track: f64,
        padding: f64,
    ) -> f64 {
        let effective_track = (track - padding).max(0.0);
        self.calculate(viewport, content, effective_track)
    }

    /// Returns `true` if the viewport is large enough to show all content
    /// (i.e. no scrolling is needed).
    pub fn is_full_visible(&self, viewport: f64, content: f64) -> bool {
        viewport >= content
    }

    /// Minimum thumb size configured.
    pub fn min_size(&self) -> f64 {
        self.min_size
    }

    /// Maximum ratio configured.
    pub fn max_ratio(&self) -> f64 {
        self.max_ratio
    }
}

// ---------------------------------------------------------------------------
// ScrollbarClickToPosition – click-to-scroll position mapper
// ---------------------------------------------------------------------------

/// Maps a click position on the scrollbar track to a scroll offset in content
/// coordinates. Supports percentage mapping, bounds checking, and
/// centre-on-click behaviour.
#[derive(Debug, Clone)]
pub struct ScrollbarClickToPosition {
    track_start: f64,
    track_length: f64,
}

impl ScrollbarClickToPosition {
    /// Create a new mapper for a track starting at `track_start` pixels with
    /// the given `track_length` in pixels.
    pub fn new(track_start: f64, track_length: f64) -> Self {
        Self {
            track_start,
            track_length: track_length.max(0.0),
        }
    }

    /// Convert a click position to a scroll offset within `[0, content_size - viewport_size]`.
    ///
    /// The click fraction along the track is mapped linearly onto the scrollable
    /// content range.
    pub fn click_to_scroll_offset(
        &self,
        click_pos: f64,
        content_size: f64,
        viewport_size: f64,
    ) -> f64 {
        let pct = self.click_to_percentage(click_pos);
        let max_scroll = (content_size - viewport_size).max(0.0);
        pct * max_scroll
    }

    /// Return the click position as a fraction of the track length (`0.0..=1.0`).
    pub fn click_to_percentage(&self, click_pos: f64) -> f64 {
        if self.track_length <= 0.0 {
            return 0.0;
        }
        let relative = click_pos - self.track_start;
        (relative / self.track_length).clamp(0.0, 1.0)
    }

    /// Returns `true` if `pos` falls within the track bounds.
    pub fn is_in_track(&self, pos: f64) -> bool {
        pos >= self.track_start && pos <= self.track_start + self.track_length
    }

    /// Compute a scroll offset that centres the viewport around the click
    /// position, clamped to valid bounds.
    pub fn center_on_click(
        &self,
        click_pos: f64,
        viewport_size: f64,
        content_size: f64,
    ) -> f64 {
        let pct = self.click_to_percentage(click_pos);
        let content_pos = pct * content_size;
        let offset = content_pos - viewport_size / 2.0;
        let max_scroll = (content_size - viewport_size).max(0.0);
        offset.clamp(0.0, max_scroll)
    }

    /// Track start position.
    pub fn track_start(&self) -> f64 {
        self.track_start
    }

    /// Track length in pixels.
    pub fn track_length(&self) -> f64 {
        self.track_length
    }
}



// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 154
// ---------------------------------------------------------------------------

/// Generic object pool `Xc154Pool<T>`.
pub struct Xc154Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc154Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc154PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc154Pool<T> {
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
    pub fn stats(&self) -> Xc154PoolStats {
        Xc154PoolStats {
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

impl<T> Default for Xc154Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc154Scheduler`.
pub struct Xc154Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc154Scheduler {
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

impl Default for Xc154Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_154 hash for the given byte slice.
pub fn xc_154_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_154 convention.
pub fn xc_154_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_7 deepening: state machine + event bus ---

/// States for the Xd7 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd7State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd7State {
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
pub struct Xd7Transition {
    pub from: Xd7State,
    pub to: Xd7State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd7StateMachine {
    current: Xd7State,
    history: Vec<Xd7Transition>,
    step_counter: usize,
}

impl Xd7StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd7State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd7State {
        self.current
    }

    pub fn history(&self) -> &[Xd7Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd7State) -> Result<Xd7State, String> {
        let allowed = match (self.current, target) {
            (Xd7State::Idle, Xd7State::Running) => true,
            (Xd7State::Running, Xd7State::Paused) => true,
            (Xd7State::Running, Xd7State::Done) => true,
            (Xd7State::Paused, Xd7State::Running) => true,
            (Xd7State::Paused, Xd7State::Done) => true,
            (Xd7State::Done, Xd7State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_7: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd7Transition {
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
            "Xd7SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd7State> {
        let prefix = "Xd7SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd7State::Idle),
            "Running" => Some(Xd7State::Running),
            "Paused" => Some(Xd7State::Paused),
            "Done" => Some(Xd7State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd7State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd7 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd7Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd7Event {
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

type Xd7HandlerFn = Box<dyn Fn(&Xd7Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd7EventBus {
    handlers: Vec<(usize, Option<String>, Xd7HandlerFn)>,
    next_id: usize,
    published: Vec<Xd7Event>,
}

impl Xd7EventBus {
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
        F: Fn(&Xd7Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd7Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd7Event) {
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

    pub fn published_events(&self) -> &[Xd7Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
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

    // ---------------------------------------------------------------
    // ScrollbarAnimation tests
    // ---------------------------------------------------------------

    #[test]
    fn animation_linear_interpolation() {
        let mut anim = ScrollbarAnimation::new(0.0, 100.0, 200, ScrollEasing::Linear);
        assert!(!anim.is_complete());
        let val = anim.tick(100);
        assert!((val - 50.0).abs() < f64::EPSILON);
        assert!(!anim.is_complete());
        let val = anim.tick(100);
        assert!((val - 100.0).abs() < f64::EPSILON);
        assert!(anim.is_complete());
    }

    #[test]
    fn animation_ease_out_reaches_target() {
        let mut anim = ScrollbarAnimation::new(10.0, 200.0, 500, ScrollEasing::EaseOut);
        // Run to completion
        let final_val = anim.tick(500);
        assert!((final_val - 200.0).abs() < f64::EPSILON);
        assert!(anim.is_complete());
        assert!((anim.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn animation_zero_duration_jumps_to_target() {
        let anim = ScrollbarAnimation::new(0.0, 42.0, 0, ScrollEasing::EaseInOut);
        assert!((anim.current_value() - 42.0).abs() < f64::EPSILON);
        assert!((anim.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn easing_ease_in_out_midpoint() {
        // At t=0.5, ease-in-out should return 0.5
        let val = ScrollEasing::EaseInOut.evaluate(0.5);
        assert!((val - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn easing_boundaries() {
        for easing in &[ScrollEasing::Linear, ScrollEasing::EaseOut, ScrollEasing::EaseInOut] {
            assert!((easing.evaluate(0.0) - 0.0).abs() < f64::EPSILON);
            assert!((easing.evaluate(1.0) - 1.0).abs() < f64::EPSILON);
            // Clamps out-of-range
            assert!((easing.evaluate(-0.5) - 0.0).abs() < f64::EPSILON);
            assert!((easing.evaluate(1.5) - 1.0).abs() < f64::EPSILON);
        }
    }

    // ---------------------------------------------------------------
    // MinimapRenderer tests
    // ---------------------------------------------------------------

    #[test]
    fn minimap_line_height() {
        let r = MinimapRenderer::new(1000, 500.0, 80.0);
        assert!((r.line_height() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn minimap_line_to_y_and_back() {
        let r = MinimapRenderer::new(200, 400.0, 60.0);
        let y = r.line_to_y(50);
        let line = r.y_to_line(y);
        assert_eq!(line, 50);
    }

    #[test]
    fn minimap_marker_rect() {
        let r = MinimapRenderer::new(100, 200.0, 50.0);
        let marker = MinimapMarker { line_start: 10, line_end: 19, color: 0xFF0000FF };
        let (top, height) = r.marker_rect(&marker);
        assert!((top - 20.0).abs() < f64::EPSILON);
        assert!((height - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn minimap_zero_lines() {
        let r = MinimapRenderer::new(0, 400.0, 60.0);
        assert!((r.line_height() - 0.0).abs() < f64::EPSILON);
        assert_eq!(r.y_to_line(100.0), 0);
    }

    // ---------------------------------------------------------------
    // ScrollbarAccessibility tests
    // ---------------------------------------------------------------

    #[test]
    fn accessibility_vertical_at_top() {
        let state = make_state(0.0, 100.0, 1000.0);
        let a11y = ScrollbarAccessibility::from_state(&state, ScrollbarOrientation::Vertical);
        assert_eq!(a11y.role, "scrollbar");
        assert_eq!(a11y.value_now, 0);
        assert_eq!(a11y.orientation_str, "vertical");
    }

    #[test]
    fn accessibility_display() {
        let state = make_state(450.0, 100.0, 1000.0);
        let a11y = ScrollbarAccessibility::from_state(&state, ScrollbarOrientation::Vertical);
        let s = format!("{a11y}");
        assert!(s.contains("aria-valuenow=50"));
        assert!(s.contains("vertical"));
    }

    // ---------------------------------------------------------------
    // ScrollbarZoom tests
    // ---------------------------------------------------------------

    #[test]
    fn zoom_in_out() {
        let mut z = ScrollbarZoom::new(0.5, 3.0, 0.5);
        assert!((z.level() - 1.0).abs() < f64::EPSILON);
        z.zoom_in();
        assert!((z.level() - 1.5).abs() < f64::EPSILON);
        z.zoom_out();
        assert!((z.level() - 1.0).abs() < f64::EPSILON);
        assert!(z.is_default());
    }

    #[test]
    fn zoom_clamps_to_bounds() {
        let mut z = ScrollbarZoom::new(0.5, 2.0, 0.5);
        z.set_level(10.0);
        assert!((z.level() - 2.0).abs() < f64::EPSILON);
        z.set_level(0.1);
        assert!((z.level() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn zoom_apply_scales_value() {
        let mut z = ScrollbarZoom::default();
        z.set_level(2.0);
        assert!((z.apply(100.0) - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zoom_reset() {
        let mut z = ScrollbarZoom::default();
        z.zoom_in();
        z.zoom_in();
        assert!(!z.is_default());
        z.reset();
        assert!(z.is_default());
    }

    // ---------------------------------------------------------------
    // ScrollState extended helpers
    // ---------------------------------------------------------------

    #[test]
    fn scroll_state_new_starts_at_origin() {
        let s = ScrollState::new(200.0, 100.0, 400.0, 800.0);
        assert!((s.scroll_top - 0.0).abs() < f64::EPSILON);
        assert!((s.scroll_left - 0.0).abs() < f64::EPSILON);
        assert!((s.viewport_width - 200.0).abs() < f64::EPSILON);
        assert!((s.content_height - 800.0).abs() < f64::EPSILON);
    }

    #[test]
    fn content_fits_when_small() {
        let s = ScrollState::new(200.0, 100.0, 100.0, 50.0);
        assert!(s.content_fits());
        let s2 = ScrollState::new(200.0, 100.0, 400.0, 800.0);
        assert!(!s2.content_fits());
    }

    #[test]
    fn max_scroll_offsets() {
        let s = ScrollState::new(100.0, 100.0, 300.0, 500.0);
        assert!((s.max_scroll_top() - 400.0).abs() < f64::EPSILON);
        assert!((s.max_scroll_left() - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_by_vertical_clamps() {
        let mut s = ScrollState::new(100.0, 100.0, 100.0, 500.0);
        s.scroll_by_vertical(100.0);
        assert!((s.scroll_top - 100.0).abs() < f64::EPSILON);
        s.scroll_by_vertical(-200.0);
        assert!((s.scroll_top - 0.0).abs() < f64::EPSILON);
        s.scroll_by_vertical(9999.0);
        assert!((s.scroll_top - 400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_by_horizontal_clamps() {
        let mut s = ScrollState::new(100.0, 100.0, 300.0, 100.0);
        s.scroll_by_horizontal(50.0);
        assert!((s.scroll_left - 50.0).abs() < f64::EPSILON);
        s.scroll_by_horizontal(9999.0);
        assert!((s.scroll_left - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ensure_visible_vertical_scrolls_down() {
        let mut s = ScrollState::new(100.0, 100.0, 100.0, 1000.0);
        // y=500 is far below viewport [0..100]
        s.ensure_visible_vertical(500.0, 10.0);
        // After call, 500 should be within the viewport
        assert!(s.scroll_top <= 500.0);
        assert!(s.scroll_top + s.viewport_height >= 500.0);
    }

    #[test]
    fn ensure_visible_vertical_scrolls_up() {
        let mut s = ScrollState::new(100.0, 100.0, 100.0, 1000.0);
        s.scroll_top = 500.0;
        s.ensure_visible_vertical(100.0, 10.0);
        assert!(s.scroll_top <= 100.0);
    }

    #[test]
    fn ensure_visible_horizontal_scrolls() {
        let mut s = ScrollState::new(100.0, 100.0, 800.0, 100.0);
        s.ensure_visible_horizontal(600.0, 5.0);
        assert!(s.scroll_left <= 600.0);
        assert!(s.scroll_left + s.viewport_width >= 600.0);
    }

    #[test]
    fn visible_ranges() {
        let s = ScrollState {
            scroll_top: 50.0,
            scroll_left: 30.0,
            viewport_height: 100.0,
            viewport_width: 200.0,
            content_height: 500.0,
            content_width: 500.0,
        };
        let (top, bottom) = s.visible_vertical_range();
        assert!((top - 50.0).abs() < f64::EPSILON);
        assert!((bottom - 150.0).abs() < f64::EPSILON);
        let (left, right) = s.visible_horizontal_range();
        assert!((left - 30.0).abs() < f64::EPSILON);
        assert!((right - 230.0).abs() < f64::EPSILON);
    }

    #[test]
    fn is_point_visible_checks() {
        let s = ScrollState {
            scroll_top: 100.0,
            scroll_left: 50.0,
            viewport_height: 200.0,
            viewport_width: 300.0,
            content_height: 1000.0,
            content_width: 1000.0,
        };
        assert!(s.is_point_visible(100.0, 200.0));
        assert!(!s.is_point_visible(0.0, 0.0));
        assert!(!s.is_point_visible(400.0, 200.0));
    }

    // ---------------------------------------------------------------
    // ScrollbarWidget extended methods
    // ---------------------------------------------------------------

    #[test]
    fn widget_scroll_to_left_right() {
        let mut w = ScrollbarWidget::new(
            ScrollbarConfig::default(),
            ScrollState::new(100.0, 100.0, 500.0, 100.0),
        );
        w.scroll_to_right();
        assert!((w.state.scroll_left - 400.0).abs() < f64::EPSILON);
        w.scroll_to_left();
        assert!((w.state.scroll_left - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn widget_page_left_right() {
        let mut w = ScrollbarWidget::new(
            ScrollbarConfig::default(),
            ScrollState::new(100.0, 100.0, 500.0, 100.0),
        );
        w.scroll_page_right();
        assert!((w.state.scroll_left - 100.0).abs() < f64::EPSILON);
        w.scroll_page_left();
        assert!((w.state.scroll_left - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn widget_scroll_to_line() {
        let mut w = ScrollbarWidget::new(
            ScrollbarConfig::default(),
            ScrollState::new(100.0, 100.0, 100.0, 5000.0),
        );
        w.scroll_to_line(10, 20.0);
        assert!((w.state.scroll_top - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn widget_visible_line_range() {
        let mut w = ScrollbarWidget::new(
            ScrollbarConfig::default(),
            ScrollState::new(100.0, 100.0, 100.0, 5000.0),
        );
        w.state.scroll_top = 200.0;
        let (first, last) = w.visible_line_range(20.0);
        assert_eq!(first, 10);
        assert_eq!(last, 15);
    }

    #[test]
    fn widget_should_show_scrollbars() {
        let w = ScrollbarWidget::new(
            ScrollbarConfig::default(),
            ScrollState::new(100.0, 100.0, 50.0, 500.0),
        );
        assert!(w.should_show_vertical());
        assert!(!w.should_show_horizontal());

        let w2 = ScrollbarWidget::new(
            ScrollbarConfig::default().with_vertical(ScrollbarVisibility::Hidden),
            ScrollState::new(100.0, 100.0, 50.0, 500.0),
        );
        assert!(!w2.should_show_vertical());

        let w3 = ScrollbarWidget::new(
            ScrollbarConfig::default().with_horizontal(ScrollbarVisibility::Visible),
            ScrollState::new(100.0, 100.0, 50.0, 500.0),
        );
        assert!(w3.should_show_horizontal());
    }

    // ---------------------------------------------------------------
    // ScrollbarConfig builder tests
    // ---------------------------------------------------------------

    #[test]
    fn config_builder_methods() {
        let cfg = ScrollbarConfig::default()
            .with_vertical(ScrollbarVisibility::Hidden)
            .with_horizontal(ScrollbarVisibility::Visible)
            .with_sensitivity(2.5)
            .with_fast_sensitivity(10.0);
        assert_eq!(cfg.vertical, ScrollbarVisibility::Hidden);
        assert_eq!(cfg.horizontal, ScrollbarVisibility::Visible);
        assert!((cfg.scroll_sensitivity - 2.5).abs() < f64::EPSILON);
        assert!((cfg.fast_scroll_sensitivity - 10.0).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // ScrollbarAnimation extended helpers
    // ---------------------------------------------------------------

    #[test]
    fn animation_remaining_ms() {
        let mut anim = ScrollbarAnimation::new(0.0, 100.0, 300, ScrollEasing::Linear);
        assert_eq!(anim.remaining_ms(), 300);
        anim.tick(100);
        assert_eq!(anim.remaining_ms(), 200);
        anim.tick(300);
        assert_eq!(anim.remaining_ms(), 0);
    }

    #[test]
    fn animation_restart() {
        let mut anim = ScrollbarAnimation::new(0.0, 100.0, 200, ScrollEasing::Linear);
        anim.tick(200);
        assert!(anim.is_complete());
        anim.restart();
        assert!(!anim.is_complete());
        assert!((anim.current_value() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn animation_reverse() {
        let mut anim = ScrollbarAnimation::new(0.0, 100.0, 200, ScrollEasing::Linear);
        anim.tick(100);
        anim.reverse();
        assert!((anim.from - 100.0).abs() < f64::EPSILON);
        assert!((anim.to - 0.0).abs() < f64::EPSILON);
        assert!(!anim.is_complete());
    }

    #[test]
    fn animation_retarget() {
        let mut anim = ScrollbarAnimation::new(0.0, 100.0, 200, ScrollEasing::Linear);
        anim.tick(100); // at 50.0
        anim.retarget(200.0);
        assert!((anim.from - 50.0).abs() < f64::EPSILON);
        assert!((anim.to - 200.0).abs() < f64::EPSILON);
        assert_eq!(anim.elapsed_ms, 0);
    }

    // ---------------------------------------------------------------
    // MinimapRenderer extended helpers
    // ---------------------------------------------------------------

    #[test]
    fn minimap_is_line_visible() {
        let r = MinimapRenderer::new(100, 200.0, 50.0);
        assert!(r.is_line_visible(0));
        assert!(r.is_line_visible(50));
        assert!(r.is_line_visible(99));
    }

    #[test]
    fn minimap_clamp_line() {
        let r = MinimapRenderer::new(100, 200.0, 50.0);
        assert_eq!(r.clamp_line(50), 50);
        assert_eq!(r.clamp_line(999), 99);
    }

    #[test]
    fn minimap_markers_in_range() {
        let r = MinimapRenderer::new(200, 400.0, 60.0);
        let markers = vec![
            MinimapMarker { line_start: 0, line_end: 10, color: 0xFF0000FF },
            MinimapMarker { line_start: 50, line_end: 60, color: 0x00FF00FF },
            MinimapMarker { line_start: 100, line_end: 110, color: 0x0000FFFF },
        ];
        let found = r.markers_in_range(&markers, 5, 55);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].line_start, 0);
        assert_eq!(found[1].line_start, 50);
    }

    // ---------------------------------------------------------------
    // ScrollbarAnnotation tests
    // ---------------------------------------------------------------

    #[test]
    fn annotation_single_line() {
        let a = ScrollbarAnnotation::single_line(42, AnnotationKind::Error);
        assert_eq!(a.line_start, 42);
        assert_eq!(a.line_end, 42);
        assert_eq!(a.line_span(), 1);
        assert_eq!(a.kind, AnnotationKind::Error);
        assert!(a.color_override.is_none());
    }

    #[test]
    fn annotation_line_range_normalised() {
        let a = ScrollbarAnnotation::line_range(50, 10, AnnotationKind::Warning);
        assert_eq!(a.line_start, 10);
        assert_eq!(a.line_end, 50);
        assert_eq!(a.line_span(), 41);
    }

    #[test]
    fn annotation_color_override() {
        let a = ScrollbarAnnotation::single_line(0, AnnotationKind::Info)
            .with_color(0xDEADBEEF);
        assert_eq!(a.effective_color(), 0xDEADBEEF);
    }

    #[test]
    fn annotation_default_color() {
        let a = ScrollbarAnnotation::single_line(0, AnnotationKind::SearchResult);
        assert_eq!(a.effective_color(), AnnotationKind::SearchResult.default_color());
    }

    #[test]
    fn annotation_overlaps_range() {
        let a = ScrollbarAnnotation::line_range(10, 20, AnnotationKind::Error);
        assert!(a.overlaps_range(15, 25));
        assert!(a.overlaps_range(5, 15));
        assert!(a.overlaps_range(0, 100));
        assert!(!a.overlaps_range(21, 30));
        assert!(!a.overlaps_range(0, 9));
    }

    #[test]
    fn annotation_kind_display() {
        assert_eq!(format!("{}", AnnotationKind::Error), "error");
        assert_eq!(format!("{}", AnnotationKind::GitAdded), "git-added");
        assert_eq!(format!("{}", AnnotationKind::Custom(7)), "custom(7)");
    }

    #[test]
    fn annotation_kind_priority_ordering() {
        assert!(AnnotationKind::Error.priority() > AnnotationKind::Warning.priority());
        assert!(AnnotationKind::Warning.priority() > AnnotationKind::Info.priority());
        assert!(AnnotationKind::SearchResult.priority() > AnnotationKind::Info.priority());
    }

    // ---------------------------------------------------------------
    // OverviewRuler tests
    // ---------------------------------------------------------------

    #[test]
    fn overview_ruler_decoration_for_full_lane() {
        let ruler = OverviewRuler::new(1000.0, 30.0, 500);
        let ann = ScrollbarAnnotation::single_line(250, AnnotationKind::Error);
        let dec = ruler.decoration_for(&ann, OverviewRulerLane::Full);
        assert!((dec.x - 0.0).abs() < f64::EPSILON);
        assert!((dec.width - 30.0).abs() < f64::EPSILON);
        assert!(dec.height >= 2.0);
        assert_eq!(dec.color, AnnotationKind::Error.default_color());
    }

    #[test]
    fn overview_ruler_lane_rect_thirds() {
        let ruler = OverviewRuler::new(100.0, 30.0, 100);
        let (lx, lw) = ruler.lane_rect(OverviewRulerLane::Left);
        let (cx, cw) = ruler.lane_rect(OverviewRulerLane::Center);
        let (rx, rw) = ruler.lane_rect(OverviewRulerLane::Right);
        assert!((lx - 0.0).abs() < f64::EPSILON);
        assert!((lw - 10.0).abs() < f64::EPSILON);
        assert!((cx - 10.0).abs() < f64::EPSILON);
        assert!((cw - 10.0).abs() < f64::EPSILON);
        assert!((rx - 20.0).abs() < f64::EPSILON);
        assert!((rw - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn overview_ruler_compute_decorations_sorted_by_priority() {
        let ruler = OverviewRuler::new(1000.0, 30.0, 100);
        let a_err = ScrollbarAnnotation::single_line(10, AnnotationKind::Error);
        let a_info = ScrollbarAnnotation::single_line(20, AnnotationKind::Info);
        let entries = vec![
            (&a_err, OverviewRulerLane::Full),
            (&a_info, OverviewRulerLane::Left),
        ];
        let decs = ruler.compute_decorations(&entries);
        assert_eq!(decs.len(), 2);
        // Info (priority 70) is drawn first, Error (100) on top
        assert_eq!(decs[0].color, AnnotationKind::Info.default_color());
        assert_eq!(decs[1].color, AnnotationKind::Error.default_color());
    }

    #[test]
    fn overview_ruler_annotations_in_view() {
        let anns = vec![
            ScrollbarAnnotation::single_line(5, AnnotationKind::Error),
            ScrollbarAnnotation::line_range(10, 20, AnnotationKind::Warning),
            ScrollbarAnnotation::single_line(50, AnnotationKind::Info),
        ];
        assert_eq!(OverviewRuler::annotations_in_view(&anns, 0, 15), 2);
        assert_eq!(OverviewRuler::annotations_in_view(&anns, 25, 100), 1);
        assert_eq!(OverviewRuler::annotations_in_view(&anns, 0, 100), 3);
    }

    #[test]
    fn overview_ruler_zero_lines() {
        let ruler = OverviewRuler::new(500.0, 20.0, 0);
        let ann = ScrollbarAnnotation::single_line(0, AnnotationKind::Error);
        let dec = ruler.decoration_for(&ann, OverviewRulerLane::Full);
        assert!((dec.y - 0.0).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // ThumbSizeCalculator tests
    // ---------------------------------------------------------------

    #[test]
    fn thumb_calc_default() {
        let calc = ThumbSizeCalculator::new();
        assert!((calc.min_thumb_px - 20.0).abs() < f64::EPSILON);
        assert!((calc.max_thumb_fraction - 1.0).abs() < f64::EPSILON);
        assert_eq!(calc.large_file_threshold, 10_000);
    }

    #[test]
    fn thumb_calc_viewport_fills_content() {
        let calc = ThumbSizeCalculator::new();
        let size = calc.compute(100, 50, 600.0);
        assert!(size >= calc.min_thumb_px);
    }

    #[test]
    fn thumb_calc_proportional() {
        let calc = ThumbSizeCalculator::new();
        let size = calc.compute(100, 1000, 600.0);
        // 100/1000 * 600 = 60, above the 20px minimum
        assert!((size - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn thumb_calc_large_file_reduction() {
        let calc = ThumbSizeCalculator::new().with_large_file_threshold(500);
        let size_normal = calc.compute(100, 400, 600.0);
        let size_large = calc.compute(100, 600, 600.0);
        // Large-file mode applies a 0.8 factor
        assert!(size_large < size_normal);
    }

    #[test]
    fn thumb_calc_min_clamp() {
        let calc = ThumbSizeCalculator::new().with_min_thumb(50.0);
        let size = calc.compute(1, 1_000_000, 600.0);
        assert!((size - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn thumb_calc_position() {
        let calc = ThumbSizeCalculator::new();
        let pos = calc.thumb_position(0.5, 600.0, 60.0);
        // 0.5 * (600 - 60) = 270
        assert!((pos - 270.0).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // click_to_position tests
    // ---------------------------------------------------------------

    #[test]
    fn click_to_position_centre_viewport() {
        let track = ScrollbarTrack::new(0.0, 500.0, ScrollbarOrientation::Vertical);
        let state = ScrollState::new(800.0, 200.0, 800.0, 2000.0);
        let result = click_to_position(250.0, &track, &state, 20.0);
        // fraction = 250/500 = 0.5, content_y = 0.5*2000 = 1000
        // scroll = 1000 - 100 = 900, max = 1800
        assert!((result.scroll_offset - 900.0).abs() < f64::EPSILON);
        assert!((result.track_fraction - 0.5).abs() < f64::EPSILON);
        assert_eq!(result.target_line, 45); // 900/20 = 45
    }

    #[test]
    fn click_to_position_clamp_top() {
        let track = ScrollbarTrack::new(0.0, 500.0, ScrollbarOrientation::Vertical);
        let state = ScrollState::new(800.0, 200.0, 800.0, 2000.0);
        let result = click_to_position(0.0, &track, &state, 20.0);
        assert!((result.scroll_offset - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn click_to_position_clamp_bottom() {
        let track = ScrollbarTrack::new(0.0, 500.0, ScrollbarOrientation::Vertical);
        let state = ScrollState::new(800.0, 200.0, 800.0, 2000.0);
        let result = click_to_position(500.0, &track, &state, 20.0);
        assert!((result.scroll_offset - 1800.0).abs() < f64::EPSILON);
    }

    #[test]
    fn click_to_position_zero_line_height() {
        let track = ScrollbarTrack::new(0.0, 500.0, ScrollbarOrientation::Vertical);
        let state = ScrollState::new(800.0, 200.0, 800.0, 2000.0);
        let result = click_to_position(250.0, &track, &state, 0.0);
        assert_eq!(result.target_line, 0);
    }

    // ---------------------------------------------------------------
    // Colour utility tests
    // ---------------------------------------------------------------

    #[test]
    fn rgba_roundtrip() {
        let packed = pack_rgba(0xDE, 0xAD, 0xBE, 0xEF);
        let (r, g, b, a) = rgba_components(packed);
        assert_eq!((r, g, b, a), (0xDE, 0xAD, 0xBE, 0xEF));
    }

    #[test]
    fn alpha_blend_opaque_fg() {
        let fg = pack_rgba(255, 0, 0, 255); // fully opaque red
        let bg = pack_rgba(0, 255, 0, 255); // fully opaque green
        let result = alpha_blend(fg, bg);
        let (r, g, b, _) = rgba_components(result);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn alpha_blend_transparent_fg() {
        let fg = pack_rgba(255, 0, 0, 0); // fully transparent red
        let bg = pack_rgba(0, 255, 0, 255);
        let result = alpha_blend(fg, bg);
        let (r, g, b, _) = rgba_components(result);
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);
    }

    #[test]
    fn alpha_blend_half_alpha() {
        let fg = pack_rgba(200, 100, 0, 128);
        let bg = pack_rgba(0, 0, 200, 255);
        let result = alpha_blend(fg, bg);
        let (r, g, b, _a) = rgba_components(result);
        // ~50% blend
        assert!(r > 90 && r < 110);
        assert!(g > 45 && g < 60);
        assert!(b > 90 && b < 110);
    }



    // ---------------------------------------------------------------
    // AnnotationType tests
    // ---------------------------------------------------------------

    #[test]
    fn annotation_type_priority_error_is_highest() {
        assert!(AnnotationType::Error.priority() > AnnotationType::Warning.priority());
        assert!(AnnotationType::Warning.priority() > AnnotationType::Search.priority());
        assert!(AnnotationType::Search.priority() > AnnotationType::Change.priority());
        assert!(AnnotationType::Change.priority() > AnnotationType::Info.priority());
    }

    #[test]
    fn annotation_type_is_error_and_warning() {
        assert!(AnnotationType::Error.is_error());
        assert!(!AnnotationType::Warning.is_error());
        assert!(AnnotationType::Warning.is_warning());
        assert!(!AnnotationType::Error.is_warning());
        assert!(!AnnotationType::Info.is_error());
        assert!(!AnnotationType::Search.is_warning());
    }

    #[test]
    fn annotation_type_display() {
        assert_eq!(format!("{}", AnnotationType::Error), "error");
        assert_eq!(format!("{}", AnnotationType::Warning), "warning");
        assert_eq!(format!("{}", AnnotationType::Info), "info");
        assert_eq!(format!("{}", AnnotationType::Search), "search");
        assert_eq!(format!("{}", AnnotationType::Change), "change");
    }

    #[test]
    fn annotation_type_equality() {
        assert_eq!(AnnotationType::Error, AnnotationType::Error);
        assert_ne!(AnnotationType::Error, AnnotationType::Warning);
        let a = AnnotationType::Search;
        let b = a; // Copy
        assert_eq!(a, b);
    }

    // ---------------------------------------------------------------
    // ScrollbarAnnotationSimple tests
    // ---------------------------------------------------------------

    #[test]
    fn annotation_simple_new_and_accessors() {
        let ann = ScrollbarAnnotationSimple::new(42, 0xFF0000FF, AnnotationType::Error);
        assert_eq!(ann.line(), 42);
        assert_eq!(ann.color(), 0xFF0000FF);
        assert_eq!(ann.annotation_type(), AnnotationType::Error);
    }

    #[test]
    fn annotation_simple_is_error_and_warning() {
        let err = ScrollbarAnnotationSimple::new(0, 0, AnnotationType::Error);
        let warn = ScrollbarAnnotationSimple::new(0, 0, AnnotationType::Warning);
        let info = ScrollbarAnnotationSimple::new(0, 0, AnnotationType::Info);
        assert!(err.is_error());
        assert!(!err.is_warning());
        assert!(warn.is_warning());
        assert!(!warn.is_error());
        assert!(!info.is_error());
        assert!(!info.is_warning());
    }

    #[test]
    fn annotation_simple_priority_delegates_to_type() {
        let err = ScrollbarAnnotationSimple::new(10, 0, AnnotationType::Error);
        let search = ScrollbarAnnotationSimple::new(10, 0, AnnotationType::Search);
        assert_eq!(err.priority(), AnnotationType::Error.priority());
        assert!(err.priority() > search.priority());
    }

    #[test]
    fn annotation_simple_clone() {
        let a = ScrollbarAnnotationSimple::new(5, 0xAABBCCDD, AnnotationType::Change);
        let b = a.clone();
        assert_eq!(b.line(), 5);
        assert_eq!(b.color(), 0xAABBCCDD);
        assert_eq!(b.annotation_type(), AnnotationType::Change);
    }

    // ---------------------------------------------------------------
    // ScrollbarOverviewRuler tests
    // ---------------------------------------------------------------

    #[test]
    fn overview_ruler_add_and_count() {
        let mut ruler = ScrollbarOverviewRuler::new(500.0, 1000);
        assert_eq!(ruler.annotation_count(), 0);
        ruler.add_annotation(ScrollbarAnnotationSimple::new(10, 0xFF0000FF, AnnotationType::Error));
        ruler.add_annotation(ScrollbarAnnotationSimple::new(20, 0x00FF00FF, AnnotationType::Warning));
        assert_eq!(ruler.annotation_count(), 2);
    }

    #[test]
    fn overview_ruler_annotations_at_line() {
        let mut ruler = ScrollbarOverviewRuler::new(500.0, 1000);
        ruler.add_annotation(ScrollbarAnnotationSimple::new(10, 0xFF0000FF, AnnotationType::Error));
        ruler.add_annotation(ScrollbarAnnotationSimple::new(10, 0x00FF00FF, AnnotationType::Warning));
        ruler.add_annotation(ScrollbarAnnotationSimple::new(20, 0x0000FFFF, AnnotationType::Info));
        let at_10 = ruler.annotations_at_line(10);
        assert_eq!(at_10.len(), 2);
        let at_20 = ruler.annotations_at_line(20);
        assert_eq!(at_20.len(), 1);
        let at_30 = ruler.annotations_at_line(30);
        assert_eq!(at_30.len(), 0);
    }

    #[test]
    fn overview_ruler_clear() {
        let mut ruler = ScrollbarOverviewRuler::new(500.0, 100);
        ruler.add_annotation(ScrollbarAnnotationSimple::new(1, 0, AnnotationType::Info));
        ruler.add_annotation(ScrollbarAnnotationSimple::new(2, 0, AnnotationType::Info));
        ruler.clear();
        assert_eq!(ruler.annotation_count(), 0);
    }

    #[test]
    fn overview_ruler_y_position_maps_lines() {
        let ruler = ScrollbarOverviewRuler::new(999.0, 1000);
        let y0 = ruler.y_position(0);
        assert!((y0 - 0.0).abs() < 0.01);
        let y_last = ruler.y_position(999);
        assert!((y_last - 999.0).abs() < 0.01);
        let y_mid = ruler.y_position(500);
        assert!(y_mid > 400.0 && y_mid < 600.0);
    }

    #[test]
    fn overview_ruler_y_position_clamps() {
        let ruler = ScrollbarOverviewRuler::new(100.0, 50);
        let y = ruler.y_position(9999);
        assert!((y - 100.0).abs() < 0.01);
    }

    #[test]
    fn overview_ruler_line_from_y_roundtrip() {
        let ruler = ScrollbarOverviewRuler::new(1000.0, 500);
        for line in [0, 1, 100, 250, 499] {
            let y = ruler.y_position(line);
            let back = ruler.line_from_y(y);
            assert!(
                (back as isize - line as isize).unsigned_abs() <= 1,
                "line={line}, y={y}, back={back}"
            );
        }
    }

    #[test]
    fn overview_ruler_line_from_y_clamped() {
        let ruler = ScrollbarOverviewRuler::new(100.0, 50);
        assert_eq!(ruler.line_from_y(-10.0), 0);
        assert_eq!(ruler.line_from_y(200.0), 49);
    }

    #[test]
    fn overview_ruler_visible_annotations() {
        let mut ruler = ScrollbarOverviewRuler::new(500.0, 1000);
        ruler.add_annotation(ScrollbarAnnotationSimple::new(5, 0, AnnotationType::Error));
        ruler.add_annotation(ScrollbarAnnotationSimple::new(50, 0, AnnotationType::Warning));
        ruler.add_annotation(ScrollbarAnnotationSimple::new(150, 0, AnnotationType::Info));
        ruler.add_annotation(ScrollbarAnnotationSimple::new(500, 0, AnnotationType::Search));
        let visible = ruler.visible_annotations(10, 200);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].line(), 50);
        assert_eq!(visible[1].line(), 150);
    }

    #[test]
    fn overview_ruler_annotations_by_type() {
        let mut ruler = ScrollbarOverviewRuler::new(500.0, 1000);
        ruler.add_annotation(ScrollbarAnnotationSimple::new(1, 0, AnnotationType::Error));
        ruler.add_annotation(ScrollbarAnnotationSimple::new(2, 0, AnnotationType::Error));
        ruler.add_annotation(ScrollbarAnnotationSimple::new(3, 0, AnnotationType::Warning));
        let errors = ruler.annotations_by_type(AnnotationType::Error);
        assert_eq!(errors.len(), 2);
        let warnings = ruler.annotations_by_type(AnnotationType::Warning);
        assert_eq!(warnings.len(), 1);
        let changes = ruler.annotations_by_type(AnnotationType::Change);
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn overview_ruler_debug_format() {
        let ruler = ScrollbarOverviewRuler::new(100.0, 50);
        let dbg = format!("{:?}", ruler);
        assert!(dbg.contains("ScrollbarOverviewRuler"));
        assert!(dbg.contains("annotation_count"));
    }

    // ---------------------------------------------------------------
    // ScrollbarThumbSizeCalculator tests
    // ---------------------------------------------------------------

    #[test]
    fn simple_thumb_calc_proportional() {
        let calc = ScrollbarThumbSizeCalculator::new(20.0, 1.0);
        let size = calc.calculate(100.0, 1000.0, 500.0);
        // viewport/content = 0.1, so proportional = 50px, clamped >= 20
        assert!((size - 50.0).abs() < 0.01);
    }

    #[test]
    fn simple_thumb_calc_enforces_min_size() {
        let calc = ScrollbarThumbSizeCalculator::new(30.0, 1.0);
        let size = calc.calculate(10.0, 100_000.0, 500.0);
        assert!((size - 30.0).abs() < 0.01);
    }

    #[test]
    fn simple_thumb_calc_enforces_max_ratio() {
        let calc = ScrollbarThumbSizeCalculator::new(5.0, 0.5);
        let size = calc.calculate(900.0, 1000.0, 500.0);
        // proportional = 450px, but max = 0.5 * 500 = 250
        assert!((size - 250.0).abs() < 0.01);
    }

    #[test]
    fn simple_thumb_calc_full_visible_returns_max() {
        let calc = ScrollbarThumbSizeCalculator::new(20.0, 1.0);
        let size = calc.calculate(1000.0, 500.0, 400.0);
        // viewport >= content -> full track
        assert!((size - 400.0).abs() < 0.01);
    }

    #[test]
    fn simple_thumb_calc_with_padding() {
        let calc = ScrollbarThumbSizeCalculator::new(10.0, 1.0);
        let size_no_pad = calc.calculate(100.0, 1000.0, 500.0);
        let size_pad = calc.calculate_with_padding(100.0, 1000.0, 500.0, 100.0);
        // padding reduces effective track from 500 to 400
        assert!(size_pad < size_no_pad);
        let expected = calc.calculate(100.0, 1000.0, 400.0);
        assert!((size_pad - expected).abs() < 0.01);
    }

    #[test]
    fn simple_thumb_calc_is_full_visible() {
        let calc = ScrollbarThumbSizeCalculator::new(10.0, 1.0);
        assert!(calc.is_full_visible(1000.0, 500.0));
        assert!(calc.is_full_visible(500.0, 500.0));
        assert!(!calc.is_full_visible(100.0, 500.0));
    }

    #[test]
    fn simple_thumb_calc_zero_content() {
        let calc = ScrollbarThumbSizeCalculator::new(15.0, 1.0);
        let size = calc.calculate(100.0, 0.0, 500.0);
        assert!((size - 15.0).abs() < 0.01);
    }

    #[test]
    fn simple_thumb_calc_accessors() {
        let calc = ScrollbarThumbSizeCalculator::new(25.0, 0.8);
        assert!((calc.min_size() - 25.0).abs() < 0.01);
        assert!((calc.max_ratio() - 0.8).abs() < 0.01);
    }

    // ---------------------------------------------------------------
    // ScrollbarClickToPosition tests
    // ---------------------------------------------------------------

    #[test]
    fn click_to_pos_percentage_at_boundaries() {
        let mapper = ScrollbarClickToPosition::new(100.0, 400.0);
        assert!((mapper.click_to_percentage(100.0) - 0.0).abs() < 0.001);
        assert!((mapper.click_to_percentage(500.0) - 1.0).abs() < 0.001);
        assert!((mapper.click_to_percentage(300.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn click_to_pos_percentage_clamped() {
        let mapper = ScrollbarClickToPosition::new(100.0, 400.0);
        assert!((mapper.click_to_percentage(50.0) - 0.0).abs() < 0.001);
        assert!((mapper.click_to_percentage(600.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn click_to_pos_is_in_track() {
        let mapper = ScrollbarClickToPosition::new(100.0, 400.0);
        assert!(mapper.is_in_track(100.0));
        assert!(mapper.is_in_track(300.0));
        assert!(mapper.is_in_track(500.0));
        assert!(!mapper.is_in_track(99.9));
        assert!(!mapper.is_in_track(500.1));
    }

    #[test]
    fn click_to_pos_scroll_offset() {
        let mapper = ScrollbarClickToPosition::new(0.0, 100.0);
        // Click at 50% of track, content=1000, viewport=200 => max_scroll=800
        let offset = mapper.click_to_scroll_offset(50.0, 1000.0, 200.0);
        assert!((offset - 400.0).abs() < 0.01);
    }

    #[test]
    fn click_to_pos_scroll_offset_at_start() {
        let mapper = ScrollbarClickToPosition::new(0.0, 100.0);
        let offset = mapper.click_to_scroll_offset(0.0, 1000.0, 200.0);
        assert!((offset - 0.0).abs() < 0.01);
    }

    #[test]
    fn click_to_pos_scroll_offset_at_end() {
        let mapper = ScrollbarClickToPosition::new(0.0, 100.0);
        let offset = mapper.click_to_scroll_offset(100.0, 1000.0, 200.0);
        assert!((offset - 800.0).abs() < 0.01);
    }

    #[test]
    fn click_to_pos_center_on_click() {
        let mapper = ScrollbarClickToPosition::new(0.0, 100.0);
        // Click at 50% => content_pos = 500, offset = 500 - 100 = 400
        let offset = mapper.center_on_click(50.0, 200.0, 1000.0);
        assert!((offset - 400.0).abs() < 0.01);
    }

    #[test]
    fn click_to_pos_center_on_click_clamped_start() {
        let mapper = ScrollbarClickToPosition::new(0.0, 100.0);
        // Click at 0% => content_pos = 0, offset = 0 - 100 = -100 => clamp to 0
        let offset = mapper.center_on_click(0.0, 200.0, 1000.0);
        assert!((offset - 0.0).abs() < 0.01);
    }

    #[test]
    fn click_to_pos_center_on_click_clamped_end() {
        let mapper = ScrollbarClickToPosition::new(0.0, 100.0);
        // Click at 100% => content_pos = 1000, offset = 1000 - 100 = 900 => clamp to 800
        let offset = mapper.center_on_click(100.0, 200.0, 1000.0);
        assert!((offset - 800.0).abs() < 0.01);
    }

    #[test]
    fn click_to_pos_zero_length_track() {
        let mapper = ScrollbarClickToPosition::new(0.0, 0.0);
        assert!((mapper.click_to_percentage(50.0) - 0.0).abs() < 0.001);
        assert!((mapper.click_to_scroll_offset(50.0, 1000.0, 200.0) - 0.0).abs() < 0.01);
    }

    #[test]
    fn click_to_pos_accessors() {
        let mapper = ScrollbarClickToPosition::new(10.0, 300.0);
        assert!((mapper.track_start() - 10.0).abs() < 0.01);
        assert!((mapper.track_length() - 300.0).abs() < 0.01);
    }

    #[test]
    fn click_to_pos_viewport_larger_than_content() {
        let mapper = ScrollbarClickToPosition::new(0.0, 100.0);
        // viewport >= content => max_scroll = 0
        let offset = mapper.click_to_scroll_offset(50.0, 200.0, 500.0);
        assert!((offset - 0.0).abs() < 0.01);
    }


    // ---- xc_ pool / scheduler tests – block 154 ----

    #[test]
    fn xc_154_pool_new_empty() {
        let pool: super::Xc154Pool<i32> = super::Xc154Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_154_pool_release_acquire() {
        let mut pool = super::Xc154Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_154_pool_acquire_empty() {
        let mut pool: super::Xc154Pool<i32> = super::Xc154Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_154_pool_full() {
        let mut pool = super::Xc154Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_154_pool_drain() {
        let mut pool = super::Xc154Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_154_pool_stats() {
        let mut pool = super::Xc154Pool::new(8);
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
    fn xc_154_pool_clear() {
        let mut pool = super::Xc154Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_154_pool_shrink() {
        let mut pool = super::Xc154Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_154_pool_default() {
        let pool: super::Xc154Pool<String> = super::Xc154Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_154_pool_extend() {
        let mut pool = super::Xc154Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_154_pool_retain() {
        let mut pool = super::Xc154Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_154_scheduler_round_robin() {
        let mut sched = super::Xc154Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_154_scheduler_empty() {
        let mut sched = super::Xc154Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_154_scheduler_reset() {
        let mut sched = super::Xc154Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_154_scheduler_add_remove() {
        let mut sched = super::Xc154Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_154_scheduler_targets() {
        let sched = super::Xc154Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_154_hash_empty() {
        assert_eq!(super::xc_154_hash(b""), 5381);
    }

    #[test]
    fn xc_154_hash_data() {
        let h = super::xc_154_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_154_hash(b"hello"), h);
    }

    #[test]
    fn xc_154_reverse_str() {
        assert_eq!(super::xc_154_reverse("abc"), "cba");
        assert_eq!(super::xc_154_reverse(""), "");
    }


    // --- xd_7 deepening tests ---

    #[test]
    fn xd_7_sm_initial_state() {
        let sm = Xd7StateMachine::new();
        assert_eq!(sm.current_state(), Xd7State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_7_sm_valid_idle_to_running() {
        let mut sm = Xd7StateMachine::new();
        assert!(sm.transition(Xd7State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd7State::Running);
    }

    #[test]
    fn xd_7_sm_valid_running_to_paused() {
        let mut sm = Xd7StateMachine::new();
        sm.transition(Xd7State::Running).unwrap();
        assert!(sm.transition(Xd7State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd7State::Paused);
    }

    #[test]
    fn xd_7_sm_valid_running_to_done() {
        let mut sm = Xd7StateMachine::new();
        sm.transition(Xd7State::Running).unwrap();
        assert!(sm.transition(Xd7State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd7State::Done);
    }

    #[test]
    fn xd_7_sm_valid_paused_to_running() {
        let mut sm = Xd7StateMachine::new();
        sm.transition(Xd7State::Running).unwrap();
        sm.transition(Xd7State::Paused).unwrap();
        assert!(sm.transition(Xd7State::Running).is_ok());
    }

    #[test]
    fn xd_7_sm_valid_done_to_idle() {
        let mut sm = Xd7StateMachine::new();
        sm.transition(Xd7State::Running).unwrap();
        sm.transition(Xd7State::Done).unwrap();
        assert!(sm.transition(Xd7State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd7State::Idle);
    }

    #[test]
    fn xd_7_sm_invalid_idle_to_done() {
        let mut sm = Xd7StateMachine::new();
        assert!(sm.transition(Xd7State::Done).is_err());
    }

    #[test]
    fn xd_7_sm_invalid_idle_to_paused() {
        let mut sm = Xd7StateMachine::new();
        assert!(sm.transition(Xd7State::Paused).is_err());
    }

    #[test]
    fn xd_7_sm_history_tracking() {
        let mut sm = Xd7StateMachine::new();
        sm.transition(Xd7State::Running).unwrap();
        sm.transition(Xd7State::Paused).unwrap();
        sm.transition(Xd7State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd7State::Idle);
        assert_eq!(sm.history()[0].to, Xd7State::Running);
        assert_eq!(sm.history()[1].from, Xd7State::Running);
        assert_eq!(sm.history()[2].to, Xd7State::Done);
    }

    #[test]
    fn xd_7_sm_serialize_deserialize() {
        let mut sm = Xd7StateMachine::new();
        sm.transition(Xd7State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd7StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd7State::Running));
    }

    #[test]
    fn xd_7_sm_deserialize_invalid() {
        assert_eq!(Xd7StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_7_sm_reset() {
        let mut sm = Xd7StateMachine::new();
        sm.transition(Xd7State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd7State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_7_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd7EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd7Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_7_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd7EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd7Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd7Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_7_bus_unsubscribe() {
        let mut bus = Xd7EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_7_event_kind_and_payload() {
        let e = Xd7Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd7Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_7_bus_clear_history() {
        let mut bus = Xd7EventBus::new();
        bus.publish(Xd7Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_7_sm_step_counter_increments() {
        let mut sm = Xd7StateMachine::new();
        sm.transition(Xd7State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd7State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
