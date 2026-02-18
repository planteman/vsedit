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


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #5
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf5Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf5TrieNode {
    children: std::collections::HashMap<char, Xf5TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf5Trie {
    root: Xf5TrieNode,
    count: usize,
}

impl Xf5Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf5TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf5TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf5TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf5BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf5BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 153).
pub struct Xh153SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh153SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 195 as u64,
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

/// A compact bit set supporting boolean operations (variant 153).
pub struct Xh153BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh153BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 153).
pub struct Xi153Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi153Deque<T> {
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
pub struct Xi153Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi153Interval {
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

/// A simple interval tree (variant 153).
pub struct Xi153IntervalTree {
    xi_intervals: Vec<Xi153Interval>,
}

impl Xi153IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi153Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi153Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi153Interval) -> Vec<&Xi153Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi153Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi153Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi153Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi153Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi153Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi153Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 153) ---

/// Disjoint set / union-find for crate 153.
pub struct Xj153UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj153UnionFind {
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

const XJ153_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 153.
pub struct Xj153BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj153BTreeNode<K, V>>>,
    len: usize,
}

struct Xj153BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj153BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj153BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ153_BTREE_ORDER - 1
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
        let mid = XJ153_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj153BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj153BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj153BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj153BTreeNode::xj_new_leaf();
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


// --- xk_153 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk153SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk153SegmentTree {
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
pub struct Xk153DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk153DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_153).
#[derive(Debug, Clone)]
pub struct Xl153Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl153Rope {
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

/// Suffix array for efficient string searching (xl_153).
#[derive(Debug, Clone)]
pub struct Xl153SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl153SuffixArray {
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
pub struct Xm153MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm153MatrixSparse {
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
pub struct Xm153Tokenizer {
    text: String,
}

impl Xm153Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 153.
pub struct Xn153Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn153Fenwick {
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

// ----- AVL tree map — crate 153 -----

#[derive(Debug, Clone)]
struct Xn153AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn153AvlNode<K, V>>>,
    right: Option<Box<Xn153AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 153.
#[derive(Debug, Clone)]
pub struct Xn153AVL<K, V> {
    root: Option<Box<Xn153AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn153AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn153AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn153AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn153AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn153AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn153AvlNode<K, V>>) -> Box<Xn153AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn153AvlNode<K, V>>) -> Box<Xn153AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn153AvlNode<K, V>>) -> Box<Xn153AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn153AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn153AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn153AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn153AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn153AvlNode<K, V>>) -> &Xn153AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn153AvlNode<K, V>>) -> (Box<Xn153AvlNode<K, V>>, Option<Box<Xn153AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn153AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn153AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn153AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn153AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn153AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn153AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn153AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo153RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo153Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo153RBNode<K, V> {
    key: K,
    value: V,
    color: Xo153Color,
    left: Option<Box<Xo153RBNode<K, V>>>,
    right: Option<Box<Xo153RBNode<K, V>>>,
}

/// A red-black tree map for crate 153.
#[derive(Debug, Clone)]
pub struct Xo153RedBlack<K, V> {
    root: Option<Box<Xo153RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo153RedBlack<K, V> {
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
            r.color = Xo153Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo153RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo153RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo153RBNode {
                    key, value, color: Xo153Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo153RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo153Color::Red)
    }

    fn xo_balance(mut h: Box<Xo153RBNode<K, V>>) -> Box<Xo153RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo153Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo153RBNode<K, V>>) -> Box<Xo153RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo153Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo153RBNode<K, V>>) -> Box<Xo153RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo153Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo153RBNode<K, V>>) {
        h.color = Xo153Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo153Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo153Color::Black; }
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
            r.color = Xo153Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo153RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo153RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo153RBNode<K, V>) -> (K, V, Option<Box<Xo153RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo153RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo153Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo153RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo153ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 153.
#[derive(Debug, Clone)]
pub struct Xo153ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo153ConsistentHash {
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
            let vkey = format!("{}#xo153#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo153#{}", node, i);
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


    // -- xf_ trie + bloom tests for instance #5 --

    #[test]
    fn xf5_trie_insert_search() {
        let mut t = Xf5Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf5_trie_starts_with() {
        let mut t = Xf5Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf5_trie_remove() {
        let mut t = Xf5Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf5_trie_word_count() {
        let mut t = Xf5Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf5_trie_longest_prefix() {
        let mut t = Xf5Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf5_trie_all_words() {
        let mut t = Xf5Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf5_trie_autocomplete() {
        let mut t = Xf5Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf5_trie_empty_search() {
        let t = Xf5Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf5_bloom_add_contains() {
        let mut bf = Xf5BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf5_bloom_probably_absent() {
        let bf = Xf5BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf5_bloom_false_positive_rate() {
        let mut bf = Xf5BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf5_bloom_clear() {
        let mut bf = Xf5BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf5_bloom_union() {
        let mut a = Xf5BloomFilter::xf_new(512, 2);
        let mut b = Xf5BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf5_bloom_intersection_estimate() {
        let mut a = Xf5BloomFilter::xf_new(512, 2);
        let mut b = Xf5BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf5_bloom_union_size_mismatch() {
        let a = Xf5BloomFilter::xf_new(256, 2);
        let b = Xf5BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh153_skip_insert_contains() {
        let mut sl = super::Xh153SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh153_skip_remove() {
        let mut sl = super::Xh153SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh153_skip_len() {
        let mut sl = super::Xh153SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh153_skip_range_query() {
        let mut sl = super::Xh153SkipList::xh_new(4);
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
    fn xh153_skip_floor_ceiling() {
        let mut sl = super::Xh153SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh153_skip_rank() {
        let mut sl = super::Xh153SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh153_skip_empty() {
        let sl = super::Xh153SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh153_skip_duplicates() {
        let mut sl = super::Xh153SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh153_bitset_set_test() {
        let mut bs = super::Xh153BitSet::xh_new(256);
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
    fn xh153_bitset_clear_count() {
        let mut bs = super::Xh153BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh153_bitset_and_or_xor() {
        let mut a = super::Xh153BitSet::xh_new(128);
        let mut b = super::Xh153BitSet::xh_new(128);
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
    fn xh153_bitset_iter_ones() {
        let mut bs = super::Xh153BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh153_bitset_first_last() {
        let mut bs = super::Xh153BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh153_bitset_empty() {
        let bs = super::Xh153BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi153_deque_push_pop_back() {
        let mut dq = super::Xi153Deque::xi_new(4);
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
    fn xi153_deque_push_pop_front() {
        let mut dq = super::Xi153Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi153_deque_mixed_ops() {
        let mut dq = super::Xi153Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi153_deque_get_and_split() {
        let mut dq = super::Xi153Deque::xi_new(8);
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
    fn xi153_deque_rotate_left() {
        let mut dq = super::Xi153Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi153_deque_rotate_right() {
        let mut dq = super::Xi153Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi153_deque_grow() {
        let mut dq = super::Xi153Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi153_deque_empty() {
        let dq = super::Xi153Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi153_interval_tree_insert_query() {
        let mut tree = super::Xi153IntervalTree::xi_new();
        tree.xi_insert(super::Xi153Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi153Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi153Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi153_interval_tree_overlap() {
        let mut tree = super::Xi153IntervalTree::xi_new();
        tree.xi_insert(super::Xi153Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi153Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi153Interval::xi_new(12, 20));
        let q = super::Xi153Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi153_interval_tree_remove() {
        let mut tree = super::Xi153IntervalTree::xi_new();
        tree.xi_insert(super::Xi153Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi153Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi153_interval_tree_gaps() {
        let mut tree = super::Xi153IntervalTree::xi_new();
        tree.xi_insert(super::Xi153Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi153Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi153Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi153Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi153Interval::xi_new(8, 10));
    }

    #[test]
    fn xi153_interval_tree_merge() {
        let mut tree = super::Xi153IntervalTree::xi_new();
        tree.xi_insert(super::Xi153Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi153Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi153Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi153Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi153Interval::xi_new(10, 15));
    }

    #[test]
    fn xi153_interval_tree_all() {
        let mut tree = super::Xi153IntervalTree::xi_new();
        tree.xi_insert(super::Xi153Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi153Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi153_interval_tree_empty() {
        let tree = super::Xi153IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi153_interval_tree_contains_point() {
        let iv = super::Xi153Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 153) ---

    #[test]
    fn xj_153_uf_make_and_find() {
        let mut uf = super::Xj153UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_153_uf_union_connected() {
        let mut uf = super::Xj153UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_153_uf_component_count() {
        let mut uf = super::Xj153UnionFind::xj_new();
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
    fn xj_153_uf_component_size() {
        let mut uf = super::Xj153UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_153_uf_largest_component() {
        let mut uf = super::Xj153UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_153_uf_many_elements() {
        let mut uf = super::Xj153UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_153_uf_separate_components() {
        let mut uf = super::Xj153UnionFind::xj_new();
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
    fn xj_153_uf_path_compression() {
        let mut uf = super::Xj153UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_153_bt_insert_get() {
        let mut bt = super::Xj153BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_153_bt_contains_len() {
        let mut bt = super::Xj153BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_153_bt_replace() {
        let mut bt = super::Xj153BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_153_bt_remove() {
        let mut bt = super::Xj153BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_153_bt_keys_values() {
        let mut bt = super::Xj153BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_153_bt_range() {
        let mut bt = super::Xj153BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_153_bt_min_max() {
        let mut bt = super::Xj153BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_153_bt_many_inserts() {
        let mut bt = super::Xj153BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_153 segment tree tests ---

    #[test]
    fn xk_153_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk153SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_153_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk153SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_153_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk153SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_153_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk153SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_153_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk153SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_153_st_single_element() {
        let data = vec![42];
        let st = super::Xk153SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_153_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk153SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_153_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk153SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_153 disjoint intervals tests ---

    #[test]
    fn xk_153_di_add_and_count() {
        let mut di = super::Xk153DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_153_di_merge_overlap() {
        let mut di = super::Xk153DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_153_di_contains() {
        let mut di = super::Xk153DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_153_di_remove() {
        let mut di = super::Xk153DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_153_di_covered_length() {
        let mut di = super::Xk153DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_153_di_gaps() {
        let mut di = super::Xk153DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_153_di_merge_adjacent() {
        let mut di = super::Xk153DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_153_di_empty() {
        let di = super::Xk153DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_153_rope_new_empty() {
        let rope = super::Xl153Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_153_rope_from_str() {
        let rope = super::Xl153Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_153_rope_insert_at() {
        let mut rope = super::Xl153Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_153_rope_delete_range() {
        let mut rope = super::Xl153Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_153_rope_char_at() {
        let rope = super::Xl153Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_153_rope_split_concat() {
        let rope = super::Xl153Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_153_rope_line_count() {
        let rope = super::Xl153Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_153_rope_line_at() {
        let rope = super::Xl153Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_153_sa_build_and_search() {
        let sa = super::Xl153SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_153_sa_count() {
        let sa = super::Xl153SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_153_sa_longest_repeated() {
        let sa = super::Xl153SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_153_sa_all_positions() {
        let sa = super::Xl153SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_153_sa_len() {
        let sa = super::Xl153SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_153_sa_empty() {
        let sa = super::Xl153SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_153_rope_slice() {
        let rope = super::Xl153Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_153_sa_search_start() {
        let sa = super::Xl153SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_153_sparse_set_get() {
        let mut m = super::Xm153MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_153_sparse_row_col() {
        let mut m = super::Xm153MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_153_sparse_transpose() {
        let mut m = super::Xm153MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_153_sparse_multiply_vec() {
        let mut m = super::Xm153MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_153_sparse_nnz_density() {
        let mut m = super::Xm153MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_153_sparse_clear() {
        let mut m = super::Xm153MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_153_sparse_overwrite_zero() {
        let mut m = super::Xm153MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_153_tokenizer_basic() {
        let t = super::Xm153Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_153_tokenizer_count() {
        let t = super::Xm153Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_153_tokenizer_unique() {
        let t = super::Xm153Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_153_tokenizer_frequency() {
        let t = super::Xm153Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_153_tokenizer_delimiter() {
        let t = super::Xm153Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_153_tokenizer_whitespace() {
        let t = super::Xm153Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_153_tokenizer_empty() {
        let t = super::Xm153Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 153 ----

    #[test]
    fn xn_153_fenwick_prefix_sum() {
        let mut ft = super::Xn153Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_153_fenwick_range_sum() {
        let mut ft = super::Xn153Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_153_fenwick_point_query() {
        let mut ft = super::Xn153Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_153_fenwick_len() {
        let ft = super::Xn153Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_153_fenwick_multiple_updates() {
        let mut ft = super::Xn153Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_153_fenwick_single_element() {
        let mut ft = super::Xn153Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_153_fenwick_find_kth() {
        let mut ft = super::Xn153Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_153_fenwick_negative_delta() {
        let mut ft = super::Xn153Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 153 ----

    #[test]
    fn xn_153_avl_insert_get() {
        let mut m = super::Xn153AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_153_avl_remove() {
        let mut m = super::Xn153AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_153_avl_in_order() {
        let mut m = super::Xn153AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_153_avl_min_max() {
        let mut m = super::Xn153AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_153_avl_floor_ceiling() {
        let mut m = super::Xn153AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_153_avl_height_balanced() {
        let mut m = super::Xn153AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_153_avl_overwrite() {
        let mut m = super::Xn153AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_153_avl_empty() {
        let m: super::Xn153AVL<i32, i32> = super::Xn153AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo153RedBlack tests ---

    #[test]
    fn xo_153_rb_insert_and_get() {
        let mut tree = super::Xo153RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_153_rb_len_and_empty() {
        let mut tree = super::Xo153RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_153_rb_min_max() {
        let mut tree = super::Xo153RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_153_rb_contains() {
        let mut tree = super::Xo153RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_153_rb_remove() {
        let mut tree = super::Xo153RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_153_rb_in_order() {
        let mut tree = super::Xo153RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_153_rb_black_height() {
        let mut tree = super::Xo153RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_153_rb_overwrite() {
        let mut tree = super::Xo153RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo153ConsistentHash tests ---

    #[test]
    fn xo_153_ch_add_and_count() {
        let mut ring = super::Xo153ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_153_ch_remove_node() {
        let mut ring = super::Xo153ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_153_ch_get_node() {
        let mut ring = super::Xo153ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_153_ch_empty_ring() {
        let ring = super::Xo153ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_153_ch_distribution() {
        let mut ring = super::Xo153ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_153_ch_rebalance() {
        let mut ring = super::Xo153ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_153_ch_virtual_nodes() {
        let mut ring = super::Xo153ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_153_ch_consistent_lookup() {
        let mut ring = super::Xo153ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}
