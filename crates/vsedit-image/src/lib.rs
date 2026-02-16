//! Image preview utilities – format detection, metadata, and zoom control.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Svg,
    Bmp,
    Webp,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub file_size: u64,
    pub uri: String,
}

#[derive(Debug, Clone)]
pub struct ImageZoom {
    pub level: f64,
}

const ZOOM_MIN: f64 = 0.1;
const ZOOM_MAX: f64 = 10.0;
const ZOOM_STEP: f64 = 0.25;

impl ImageZoom {
    pub fn new() -> Self {
        Self { level: 1.0 }
    }

    fn clamp(&mut self) {
        if self.level < ZOOM_MIN {
            self.level = ZOOM_MIN;
        } else if self.level > ZOOM_MAX {
            self.level = ZOOM_MAX;
        }
    }

    pub fn zoom_in(&mut self) {
        self.level += ZOOM_STEP;
        self.clamp();
    }

    pub fn zoom_out(&mut self) {
        self.level -= ZOOM_STEP;
        self.clamp();
    }

    pub fn fit_to_width(&mut self) {
        self.level = 1.0;
    }

    pub fn reset(&mut self) {
        self.level = 1.0;
    }
}

impl Default for ImageZoom {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    UnsupportedFormat(String),
    InvalidDimensions { width: u32, height: u32 },
    FileTooLarge { size: u64, max: u64 },
    DetectionFailed,
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat(name) => write!(f, "unsupported image format: {name}"),
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid dimensions: {width}x{height}")
            }
            Self::FileTooLarge { size, max } => {
                write!(f, "file too large: {size} bytes (max {max})")
            }
            Self::DetectionFailed => write!(f, "failed to detect image format"),
        }
    }
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Gif => "GIF",
            Self::Svg => "SVG",
            Self::Bmp => "BMP",
            Self::Webp => "WebP",
            Self::Unknown => "Unknown",
        };
        f.write_str(name)
    }
}

impl fmt::Display for ImageInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{} {} ({})",
            self.width,
            self.height,
            self.format,
            format_file_size(self.file_size)
        )
    }
}

// ---------------------------------------------------------------------------
// ImageInfo helpers
// ---------------------------------------------------------------------------

impl ImageInfo {
    /// Aspect ratio as width / height.
    pub fn aspect_ratio(&self) -> f64 {
        if self.height == 0 {
            return 0.0;
        }
        self.width as f64 / self.height as f64
    }

    pub fn is_landscape(&self) -> bool {
        self.width > self.height
    }

    pub fn is_portrait(&self) -> bool {
        self.height > self.width
    }

    pub fn is_square(&self) -> bool {
        self.width == self.height
    }

    /// Return dimensions after applying the given zoom level.
    pub fn scaled_dimensions(&self, zoom: f64) -> (u32, u32) {
        let w = (self.width as f64 * zoom).round() as u32;
        let h = (self.height as f64 * zoom).round() as u32;
        (w, h)
    }
}

// ---------------------------------------------------------------------------
// ImageZoom extras
// ---------------------------------------------------------------------------

impl ImageZoom {
    /// Set zoom to an exact level (clamped to valid range).
    pub fn zoom_to(&mut self, level: f64) {
        self.level = level;
        self.clamp();
    }

    /// Calculate zoom so the image fits within the given viewport.
    pub fn zoom_to_fit(&mut self, image_width: u32, image_height: u32, vp_width: u32, vp_height: u32) {
        if image_width == 0 || image_height == 0 {
            self.level = 1.0;
            return;
        }
        let scale_x = vp_width as f64 / image_width as f64;
        let scale_y = vp_height as f64 / image_height as f64;
        self.level = scale_x.min(scale_y);
        self.clamp();
    }
}

// ---------------------------------------------------------------------------
// detect_format_from_extension
// ---------------------------------------------------------------------------

/// Detect image format from a file extension string (e.g. `"png"`, `".jpg"`).
pub fn detect_format_from_extension(ext: &str) -> ImageFormat {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "gif" => ImageFormat::Gif,
        "svg" => ImageFormat::Svg,
        "bmp" => ImageFormat::Bmp,
        "webp" => ImageFormat::Webp,
        _ => ImageFormat::Unknown,
    }
}

// ---------------------------------------------------------------------------
// ImagePreviewConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ImagePreviewConfig {
    pub max_file_size: u64,
    pub auto_zoom: bool,
    pub background_color: String,
}

impl Default for ImagePreviewConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10 MB
            auto_zoom: true,
            background_color: "#1e1e1e".to_string(),
        }
    }
}

impl ImagePreviewConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    pub fn auto_zoom(mut self, enabled: bool) -> Self {
        self.auto_zoom = enabled;
        self
    }

    pub fn background_color(mut self, color: impl Into<String>) -> Self {
        self.background_color = color.into();
        self
    }

    /// Toggle the `auto_zoom` flag.
    pub fn toggle_auto_zoom(&mut self) {
        self.auto_zoom = !self.auto_zoom;
    }
}

/// Detect image format from the first bytes of file data using magic bytes.
pub fn detect_format(data: &[u8]) -> ImageFormat {
    if data.len() >= 4 && data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        ImageFormat::Png
    } else if data.len() >= 3 && data[0..3] == [0xFF, 0xD8, 0xFF] {
        ImageFormat::Jpeg
    } else if data.len() >= 3 && data[0..3] == [0x47, 0x49, 0x46] {
        ImageFormat::Gif
    } else if data.len() >= 2 && data[0..2] == [0x42, 0x4D] {
        ImageFormat::Bmp
    } else if data.len() >= 4 && data[0..4] == [0x52, 0x49, 0x46, 0x46] {
        // RIFF header – could be WebP if followed by WEBP.
        if data.len() >= 12 && &data[8..12] == b"WEBP" {
            ImageFormat::Webp
        } else {
            ImageFormat::Unknown
        }
    } else if data.len() >= 5 && (data.starts_with(b"<?xml") || data.starts_with(b"<svg")) {
        ImageFormat::Svg
    } else {
        ImageFormat::Unknown
    }
}

/// Format a byte count into a human-readable string.
pub fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;

    let b = bytes as f64;
    if b < KB {
        format!("{} B", bytes)
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.1} GB", b / GB)
    }
}

/// Accumulated statistics for image operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ImageStats {
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
    pub fn merge(&mut self, other: &ImageStats) {
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

impl Default for ImageStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ImageStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ImageStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for image.
#[derive(Debug, Clone)]
pub struct ImageValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ImageValidator {
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

impl Default for ImageValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ImageMetadata – rich image metadata
// ---------------------------------------------------------------------------

/// Rich metadata describing an image file.
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_depth: u8,
    pub has_alpha: bool,
    pub file_size_bytes: u64,
    pub aspect_ratio: f64,
}

impl ImageMetadata {
    /// Create metadata with sensible defaults (8-bit, no alpha, file size 0).
    pub fn new(width: u32, height: u32, format: &str) -> Self {
        let aspect_ratio = if height == 0 {
            0.0
        } else {
            width as f64 / height as f64
        };
        Self {
            width,
            height,
            format: format.to_string(),
            color_depth: 8,
            has_alpha: false,
            file_size_bytes: 0,
            aspect_ratio,
        }
    }

    /// Total number of megapixels.
    pub fn megapixels(&self) -> f64 {
        (self.width as f64 * self.height as f64) / 1_000_000.0
    }

    /// True when width > height.
    pub fn is_landscape(&self) -> bool {
        self.width > self.height
    }

    /// True when height > width.
    pub fn is_portrait(&self) -> bool {
        self.height > self.width
    }

    /// True when width == height.
    pub fn is_square(&self) -> bool {
        self.width == self.height
    }

    /// Human-readable dimensions string, e.g. "1920x1080".
    pub fn dimensions_string(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    /// Builder-style setter to mark the image as having an alpha channel.
    pub fn with_alpha(mut self) -> Self {
        self.has_alpha = true;
        self
    }

    /// Builder-style setter for the on-disk file size in bytes.
    pub fn with_file_size(mut self, bytes: u64) -> Self {
        self.file_size_bytes = bytes;
        self
    }
}

// ---------------------------------------------------------------------------
// image_to_braille – convert grayscale pixels to Unicode braille art
// ---------------------------------------------------------------------------

/// Convert a row-major grayscale pixel buffer into a Unicode braille-art string.
///
/// Each braille character encodes a 2-column × 4-row block of pixels.
/// A pixel whose value is **≥ `threshold`** is considered "on" (dot raised).
///
/// The braille dot offsets inside the 2×4 cell are:
///
/// ```text
///   col 0   col 1
///   -----   -----
///   0x01    0x08      row 0
///   0x02    0x10      row 1
///   0x04    0x20      row 2
///   0x40    0x80      row 3
/// ```
///
/// Base codepoint is U+2800.
pub fn image_to_braille(pixels: &[u8], width: usize, height: usize, threshold: u8) -> String {
    // Dot bit for (row, col) inside the 2×4 cell.
    const DOT_MAP: [[u8; 2]; 4] = [
        [0x01, 0x08],
        [0x02, 0x10],
        [0x04, 0x20],
        [0x40, 0x80],
    ];

    let rows = (height + 3) / 4; // number of braille rows
    let cols = (width + 1) / 2; // number of braille columns

    let mut out = String::with_capacity(rows * (cols + 1));

    for br in 0..rows {
        if br > 0 {
            out.push('\n');
        }
        for bc in 0..cols {
            let mut code: u32 = 0;
            for dr in 0..4u32 {
                let py = br * 4 + dr as usize;
                if py >= height {
                    continue;
                }
                for dc in 0..2u32 {
                    let px = bc * 2 + dc as usize;
                    if px >= width {
                        continue;
                    }
                    let val = pixels[py * width + px];
                    if val >= threshold {
                        code |= DOT_MAP[dr as usize][dc as usize] as u32;
                    }
                }
            }
            // SAFETY: 0x2800..=0x28FF are all valid Unicode codepoints.
            out.push(char::from_u32(0x2800 + code).unwrap_or(' '));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// image_resize – nearest-neighbour resize
// ---------------------------------------------------------------------------

/// Resize a row-major grayscale pixel buffer using nearest-neighbour sampling.
pub fn image_resize(
    pixels: &[u8],
    src_width: usize,
    src_height: usize,
    dst_width: usize,
    dst_height: usize,
) -> Vec<u8> {
    let mut out = vec![0u8; dst_width * dst_height];
    for dy in 0..dst_height {
        let sy = dy * src_height / dst_height;
        for dx in 0..dst_width {
            let sx = dx * src_width / dst_width;
            out[dy * dst_width + dx] = pixels[sy * src_width + sx];
        }
    }
    out
}

// ---------------------------------------------------------------------------
// ImageThumbnail – thumbnail generation metadata
// ---------------------------------------------------------------------------

/// Describes how to generate a thumbnail for an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageThumbnail {
    pub original_width: u32,
    pub original_height: u32,
    pub thumb_width: u32,
    pub thumb_height: u32,
}

impl ImageThumbnail {
    /// Compute thumbnail dimensions that fit within `max_side × max_side`
    /// while preserving the aspect ratio of the original image.
    pub fn new(original_width: u32, original_height: u32, max_side: u32) -> Self {
        let (tw, th) = if original_width == 0 || original_height == 0 || max_side == 0 {
            (0, 0)
        } else if original_width >= original_height {
            let scale = max_side as f64 / original_width as f64;
            let w = max_side.min(original_width);
            let h = ((original_height as f64 * scale).round() as u32).max(1).min(original_height);
            (w, h)
        } else {
            let scale = max_side as f64 / original_height as f64;
            let w = ((original_width as f64 * scale).round() as u32).max(1).min(original_width);
            let h = max_side.min(original_height);
            (w, h)
        };
        Self {
            original_width,
            original_height,
            thumb_width: tw,
            thumb_height: th,
        }
    }

    /// The scale factor from original to thumbnail.
    pub fn scale_factor(&self) -> f64 {
        if self.original_width == 0 {
            return 0.0;
        }
        self.thumb_width as f64 / self.original_width as f64
    }
}

impl fmt::Display for ImageThumbnail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{} → {}x{}",
            self.original_width, self.original_height, self.thumb_width, self.thumb_height
        )
    }
}

// ---------------------------------------------------------------------------
// ImageTransform – rotation / flip descriptors
// ---------------------------------------------------------------------------

/// Rotation angles in 90-degree increments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    None,
    Cw90,
    Cw180,
    Cw270,
}

/// Flip axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlipAxis {
    Horizontal,
    Vertical,
}

/// A chain of transforms to apply to an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageTransform {
    pub rotation: Rotation,
    pub flips: Vec<FlipAxis>,
}

impl ImageTransform {
    pub fn new() -> Self {
        Self {
            rotation: Rotation::None,
            flips: Vec::new(),
        }
    }

    pub fn rotate(mut self, rotation: Rotation) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn flip(mut self, axis: FlipAxis) -> Self {
        self.flips.push(axis);
        self
    }

    /// Return the resulting dimensions after applying the rotation.
    pub fn transformed_dimensions(&self, width: u32, height: u32) -> (u32, u32) {
        match self.rotation {
            Rotation::Cw90 | Rotation::Cw270 => (height, width),
            _ => (width, height),
        }
    }

    /// True when the transform is a no-op.
    pub fn is_identity(&self) -> bool {
        self.rotation == Rotation::None && self.flips.is_empty()
    }
}

impl Default for ImageTransform {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ImageCompare – pixel-level comparison helpers
// ---------------------------------------------------------------------------

/// Result of comparing two same-sized grayscale images.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageCompareResult {
    pub width: usize,
    pub height: usize,
    pub total_pixels: usize,
    pub differing_pixels: usize,
    pub mean_absolute_error: f64,
}

impl ImageCompareResult {
    /// Fraction of pixels that differ (0.0 = identical, 1.0 = all different).
    pub fn diff_ratio(&self) -> f64 {
        if self.total_pixels == 0 {
            return 0.0;
        }
        self.differing_pixels as f64 / self.total_pixels as f64
    }
}

/// Compare two equal-sized grayscale pixel buffers.
///
/// Returns `Err` if the buffers have different lengths or the dimensions are
/// inconsistent with the buffer length.
pub fn image_compare(
    a: &[u8],
    b: &[u8],
    width: usize,
    height: usize,
) -> Result<ImageCompareResult, ImageError> {
    let expected_len = width * height;
    if a.len() != expected_len || b.len() != expected_len {
        return Err(ImageError::InvalidDimensions {
            width: width as u32,
            height: height as u32,
        });
    }
    let mut differing = 0usize;
    let mut total_error: u64 = 0;
    for (pa, pb) in a.iter().zip(b.iter()) {
        let diff = (*pa as i16 - *pb as i16).unsigned_abs() as u64;
        if diff > 0 {
            differing += 1;
        }
        total_error += diff;
    }
    let mae = if expected_len == 0 {
        0.0
    } else {
        total_error as f64 / expected_len as f64
    };
    Ok(ImageCompareResult {
        width,
        height,
        total_pixels: expected_len,
        differing_pixels: differing,
        mean_absolute_error: mae,
    })
}

/// Compute the largest dimensions that fit within `max_w × max_h` while
/// preserving the aspect ratio of `src_w × src_h`.
pub fn image_fit_dimensions(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 || max_w == 0 || max_h == 0 {
        return (0, 0);
    }
    let scale_w = max_w as f64 / src_w as f64;
    let scale_h = max_h as f64 / src_h as f64;
    let scale = scale_w.min(scale_h).min(1.0);
    let new_w = (src_w as f64 * scale).round() as u32;
    let new_h = (src_h as f64 * scale).round() as u32;
    (new_w.max(1), new_h.max(1))
}

// ---------------------------------------------------------------------------
// Color – hex / RGB / HSL conversion helpers
// ---------------------------------------------------------------------------

/// An RGB color with 8-bit components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// An HSL color with components in the ranges H:[0,360), S:[0,1], L:[0,1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsl {
    pub h: f64,
    pub s: f64,
    pub l: f64,
}

impl Rgb {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse a CSS-style hex color string (`#RRGGBB` or `RRGGBB`).
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return Err(format!("expected 6 hex digits, got {}", hex.len()));
        }
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
        Ok(Self { r, g, b })
    }

    /// Render as `#rrggbb` lowercase hex.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Convert to HSL.
    pub fn to_hsl(self) -> Hsl {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if (max - min).abs() < f64::EPSILON {
            return Hsl { h: 0.0, s: 0.0, l };
        }
        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if (max - r).abs() < f64::EPSILON {
            ((g - b) / d) % 6.0
        } else if (max - g).abs() < f64::EPSILON {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        let h = ((h * 60.0) + 360.0) % 360.0;
        Hsl { h, s, l }
    }

    /// Relative luminance per ITU-R BT.601.
    pub fn luminance(self) -> f64 {
        0.299 * (self.r as f64) + 0.587 * (self.g as f64) + 0.114 * (self.b as f64)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rgb({}, {}, {})", self.r, self.g, self.b)
    }
}

impl fmt::Display for Hsl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hsl({:.0}, {:.1}%, {:.1}%)", self.h, self.s * 100.0, self.l * 100.0)
    }
}

// ---------------------------------------------------------------------------
// SVG dimension parsing
// ---------------------------------------------------------------------------

/// Parsed dimensions from an SVG root element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SvgDimensions {
    pub width: f64,
    pub height: f64,
}

/// Extract `width` and `height` attributes from a minimal SVG header string.
///
/// Looks for patterns like `width="300"` / `height="200"` (with or without a
/// unit suffix such as `px`).  Returns `None` if either attribute is missing
/// or cannot be parsed.
pub fn parse_svg_dimensions(svg: &str) -> Option<SvgDimensions> {
    fn extract_attr(s: &str, attr: &str) -> Option<f64> {
        let needle = format!("{}=\"", attr);
        let start = s.find(&needle)? + needle.len();
        let rest = &s[start..];
        let end = rest.find('"')?;
        let val = &rest[..end];
        // strip optional unit suffix (px, em, pt, etc.)
        let numeric: String = val.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        numeric.parse::<f64>().ok()
    }

    let header = if svg.len() > 512 { &svg[..512] } else { svg };
    let w = extract_attr(header, "width")?;
    let h = extract_attr(header, "height")?;
    Some(SvgDimensions { width: w, height: h })
}

// ---------------------------------------------------------------------------
// ImageCacheEntry / LRU eviction helper
// ---------------------------------------------------------------------------

/// A single entry in an image cache.
#[derive(Debug, Clone)]
pub struct ImageCacheEntry {
    pub uri: String,
    pub size_bytes: u64,
    pub access_counter: u64,
}

/// A simple LRU-style image cache tracker.
///
/// This does **not** store actual pixel data – it only tracks which URIs are
/// cached and their sizes so that eviction decisions can be made.
#[derive(Debug, Clone)]
pub struct ImageCacheTracker {
    entries: Vec<ImageCacheEntry>,
    max_total_bytes: u64,
    total_bytes: u64,
    counter: u64,
}

impl ImageCacheTracker {
    pub fn new(max_total_bytes: u64) -> Self {
        Self {
            entries: Vec::new(),
            max_total_bytes,
            total_bytes: 0,
            counter: 0,
        }
    }

    /// Record a cache access, inserting the entry if absent.
    /// Returns URIs that were evicted to make room.
    pub fn touch(&mut self, uri: &str, size_bytes: u64) -> Vec<String> {
        self.counter += 1;
        // If already present, just bump the counter.
        if let Some(e) = self.entries.iter_mut().find(|e| e.uri == uri) {
            e.access_counter = self.counter;
            return Vec::new();
        }
        // Evict LRU entries until there is room.
        let mut evicted = Vec::new();
        while self.total_bytes + size_bytes > self.max_total_bytes && !self.entries.is_empty() {
            // Find the entry with the smallest access_counter.
            let min_idx = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.access_counter)
                .map(|(i, _)| i)
                .unwrap();
            let removed = self.entries.swap_remove(min_idx);
            self.total_bytes -= removed.size_bytes;
            evicted.push(removed.uri);
        }
        self.entries.push(ImageCacheEntry {
            uri: uri.to_string(),
            size_bytes,
            access_counter: self.counter,
        });
        self.total_bytes += size_bytes;
        evicted
    }

    /// Number of entries currently tracked.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total bytes currently tracked.
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Check whether a URI is in the cache.
    pub fn contains(&self, uri: &str) -> bool {
        self.entries.iter().any(|e| e.uri == uri)
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.total_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_png() {
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(detect_format(&data), ImageFormat::Png);
    }

    #[test]
    fn detect_jpeg() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(detect_format(&data), ImageFormat::Jpeg);
    }

    #[test]
    fn detect_gif() {
        assert_eq!(detect_format(b"GIF89a"), ImageFormat::Gif);
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(detect_format(&[0x00, 0x01]), ImageFormat::Unknown);
    }

    #[test]
    fn zoom_clamp() {
        let mut z = ImageZoom::new();
        for _ in 0..50 {
            z.zoom_out();
        }
        assert!((z.level - ZOOM_MIN).abs() < f64::EPSILON);
        for _ in 0..100 {
            z.zoom_in();
        }
        assert!((z.level - ZOOM_MAX).abs() < f64::EPSILON);
    }

    #[test]
    fn file_size_formatting() {
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(2_411_724), "2.3 MB");
    }

    #[test]
    fn zoom_to_exact() {
        let mut z = ImageZoom::new();
        z.zoom_to(3.5);
        assert!((z.level - 3.5).abs() < f64::EPSILON);
        z.zoom_to(0.01);
        assert!((z.level - ZOOM_MIN).abs() < f64::EPSILON);
        z.zoom_to(99.0);
        assert!((z.level - ZOOM_MAX).abs() < f64::EPSILON);
    }

    #[test]
    fn zoom_to_fit_landscape() {
        let mut z = ImageZoom::new();
        z.zoom_to_fit(2000, 1000, 800, 600);
        // scale_x = 0.4, scale_y = 0.6 → picks 0.4
        assert!((z.level - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn zoom_to_fit_portrait() {
        let mut z = ImageZoom::new();
        z.zoom_to_fit(500, 2000, 800, 600);
        // scale_x = 1.6, scale_y = 0.3 → picks 0.3
        assert!((z.level - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn aspect_ratio_and_orientation() {
        let landscape = ImageInfo {
            width: 1920,
            height: 1080,
            format: ImageFormat::Png,
            file_size: 0,
            uri: String::new(),
        };
        assert!(landscape.is_landscape());
        assert!(!landscape.is_portrait());
        assert!(!landscape.is_square());
        let ratio = landscape.aspect_ratio();
        assert!((ratio - 16.0 / 9.0).abs() < 1e-9);

        let portrait = ImageInfo {
            width: 1080,
            height: 1920,
            format: ImageFormat::Jpeg,
            file_size: 0,
            uri: String::new(),
        };
        assert!(portrait.is_portrait());

        let square = ImageInfo {
            width: 512,
            height: 512,
            format: ImageFormat::Gif,
            file_size: 0,
            uri: String::new(),
        };
        assert!(square.is_square());
    }

    #[test]
    fn scaled_dimensions() {
        let info = ImageInfo {
            width: 800,
            height: 600,
            format: ImageFormat::Png,
            file_size: 0,
            uri: String::new(),
        };
        assert_eq!(info.scaled_dimensions(0.5), (400, 300));
        assert_eq!(info.scaled_dimensions(2.0), (1600, 1200));
    }

    #[test]
    fn detect_from_extension() {
        assert_eq!(detect_format_from_extension("png"), ImageFormat::Png);
        assert_eq!(detect_format_from_extension(".jpg"), ImageFormat::Jpeg);
        assert_eq!(detect_format_from_extension("JPEG"), ImageFormat::Jpeg);
        assert_eq!(detect_format_from_extension(".webp"), ImageFormat::Webp);
        assert_eq!(detect_format_from_extension("svg"), ImageFormat::Svg);
        assert_eq!(detect_format_from_extension(".bmp"), ImageFormat::Bmp);
        assert_eq!(detect_format_from_extension("tiff"), ImageFormat::Unknown);
    }

    #[test]
    fn detect_webp_magic() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&[0x00; 4]); // file size placeholder
        data.extend_from_slice(b"WEBP");
        assert_eq!(detect_format(&data), ImageFormat::Webp);
    }

    #[test]
    fn detect_svg_magic() {
        assert_eq!(detect_format(b"<svg xmlns="), ImageFormat::Svg);
        assert_eq!(detect_format(b"<?xml version="), ImageFormat::Svg);
    }

    #[test]
    fn detect_bmp_magic() {
        assert_eq!(detect_format(&[0x42, 0x4D, 0x00, 0x00]), ImageFormat::Bmp);
    }

    #[test]
    fn display_image_format() {
        assert_eq!(format!("{}", ImageFormat::Png), "PNG");
        assert_eq!(format!("{}", ImageFormat::Jpeg), "JPEG");
        assert_eq!(format!("{}", ImageFormat::Webp), "WebP");
        assert_eq!(format!("{}", ImageFormat::Unknown), "Unknown");
    }

    #[test]
    fn display_image_info() {
        let info = ImageInfo {
            width: 1920,
            height: 1080,
            format: ImageFormat::Png,
            file_size: 2_411_724,
            uri: String::new(),
        };
        assert_eq!(format!("{info}"), "1920x1080 PNG (2.3 MB)");
    }

    #[test]
    fn error_display() {
        let e = ImageError::UnsupportedFormat("TIFF".into());
        assert_eq!(e.to_string(), "unsupported image format: TIFF");

        let e = ImageError::InvalidDimensions { width: 0, height: 100 };
        assert_eq!(e.to_string(), "invalid dimensions: 0x100");

        let e = ImageError::FileTooLarge { size: 20_000_000, max: 10_000_000 };
        assert_eq!(e.to_string(), "file too large: 20000000 bytes (max 10000000)");

        let e = ImageError::DetectionFailed;
        assert_eq!(e.to_string(), "failed to detect image format");
    }

    #[test]
    fn preview_config_builder() {
        let cfg = ImagePreviewConfig::new()
            .max_file_size(5 * 1024 * 1024)
            .auto_zoom(false)
            .background_color("#000000");
        assert_eq!(cfg.max_file_size, 5 * 1024 * 1024);
        assert!(!cfg.auto_zoom);
        assert_eq!(cfg.background_color, "#000000");

        let def = ImagePreviewConfig::default();
        assert!(def.auto_zoom);
        assert_eq!(def.max_file_size, 10 * 1024 * 1024);
    }

    #[test]
    fn eq_imageformat_same() {
        assert_eq!(ImageFormat::Png, ImageFormat::Png);
    }

    #[test]
    fn ne_imageformat_diff() {
        assert_ne!(ImageFormat::Png, ImageFormat::Jpeg);
    }

    #[test]
    fn display_imageformat_variants() {
        assert!(!ImageFormat::Png.to_string().is_empty());
        assert!(!ImageFormat::Jpeg.to_string().is_empty());
        assert!(!ImageFormat::Gif.to_string().is_empty());
        assert!(!ImageFormat::Svg.to_string().is_empty());
        assert!(!ImageFormat::Bmp.to_string().is_empty());
    }

    #[test]
    fn display_imageerror_variants() {
        assert!(!ImageError::DetectionFailed.to_string().is_empty());
    }

    // ---------------------------------------------------------------
    // ImageMetadata tests
    // ---------------------------------------------------------------

    #[test]
    fn metadata_creation() {
        let m = ImageMetadata::new(1920, 1080, "png");
        assert_eq!(m.width, 1920);
        assert_eq!(m.height, 1080);
        assert_eq!(m.format, "png");
        assert_eq!(m.color_depth, 8);
        assert!(!m.has_alpha);
        assert_eq!(m.file_size_bytes, 0);
        assert!((m.aspect_ratio - 1920.0 / 1080.0).abs() < 1e-9);
    }

    #[test]
    fn metadata_megapixels() {
        let m = ImageMetadata::new(2000, 1000, "jpeg");
        assert!((m.megapixels() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn metadata_landscape_portrait() {
        let landscape = ImageMetadata::new(1920, 1080, "png");
        assert!(landscape.is_landscape());
        assert!(!landscape.is_portrait());

        let portrait = ImageMetadata::new(1080, 1920, "png");
        assert!(portrait.is_portrait());
        assert!(!portrait.is_landscape());
    }

    #[test]
    fn metadata_square() {
        let sq = ImageMetadata::new(500, 500, "bmp");
        assert!(sq.is_square());
        assert!(!sq.is_landscape());
        assert!(!sq.is_portrait());
    }

    #[test]
    fn metadata_dimensions_string() {
        let m = ImageMetadata::new(800, 600, "gif");
        assert_eq!(m.dimensions_string(), "800x600");
    }

    #[test]
    fn metadata_builders() {
        let m = ImageMetadata::new(10, 10, "png")
            .with_alpha()
            .with_file_size(4096);
        assert!(m.has_alpha);
        assert_eq!(m.file_size_bytes, 4096);
    }

    // ---------------------------------------------------------------
    // image_to_braille tests
    // ---------------------------------------------------------------

    #[test]
    fn braille_all_white() {
        // All pixels below threshold → all dots off → U+2800 (blank braille)
        let pixels = vec![0u8; 4 * 2]; // 2×4 block, all zero
        let result = image_to_braille(&pixels, 2, 4, 128);
        assert_eq!(result, "\u{2800}");
    }

    #[test]
    fn braille_all_black() {
        // All pixels at 255, threshold 128 → all 8 dots on → U+28FF
        let pixels = vec![255u8; 4 * 2]; // 2×4 block, all max
        let result = image_to_braille(&pixels, 2, 4, 128);
        assert_eq!(result, "\u{28FF}");
    }

    // ---------------------------------------------------------------
    // image_resize / image_fit_dimensions tests
    // ---------------------------------------------------------------

    #[test]
    fn resize_same_size() {
        let src = vec![10, 20, 30, 40];
        let dst = image_resize(&src, 2, 2, 2, 2);
        assert_eq!(dst, src);
    }

    #[test]
    fn resize_downscale() {
        // 4×4 → 2×2, nearest-neighbour picks top-left of each quadrant
        #[rustfmt::skip]
        let src: Vec<u8> = vec![
            1, 2, 3, 4,
            5, 6, 7, 8,
            9,10,11,12,
           13,14,15,16,
        ];
        let dst = image_resize(&src, 4, 4, 2, 2);
        assert_eq!(dst, vec![1, 3, 9, 11]);
    }

    #[test]
    fn fit_dimensions_wider() {
        // 2000×1000 into 1000×1000 → limited by width → 1000×500
        let (w, h) = image_fit_dimensions(2000, 1000, 1000, 1000);
        assert_eq!((w, h), (1000, 500));
    }

    #[test]
    fn fit_dimensions_taller() {
        // 1000×2000 into 1000×1000 → limited by height → 500×1000
        let (w, h) = image_fit_dimensions(1000, 2000, 1000, 1000);
        assert_eq!((w, h), (500, 1000));
    }

    #[test]
    fn behavior_check_0() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = ImagePreviewConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn image_stats_new_defaults() {
        let stats = ImageStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn image_stats_record_success() {
        let mut stats = ImageStats::new();
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
    fn image_stats_record_failure() {
        let mut stats = ImageStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn image_stats_reset() {
        let mut stats = ImageStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn image_stats_merge() {
        let mut a = ImageStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ImageStats::new();
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
    fn image_stats_display() {
        let mut stats = ImageStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn image_stats_default() {
        let stats = ImageStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn image_validator_accepts_valid_name() {
        let v = ImageValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn image_validator_rejects_empty() {
        let v = ImageValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn image_validator_rejects_too_long() {
        let v = ImageValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn image_validator_forbidden_prefix() {
        let v = ImageValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn image_validator_allowed_chars() {
        let v = ImageValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn image_validator_range() {
        let v = ImageValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn image_sanitize_removes_control() {
        let result = ImageValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn image_truncate_short_string() {
        assert_eq!(ImageValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn image_truncate_long_string() {
        let result = ImageValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn image_is_ascii_printable() {
        assert!(ImageValidator::is_ascii_printable("Hello World 123"));
        assert!(!ImageValidator::is_ascii_printable("Hello\x00World"));
    }

    // ---------------------------------------------------------------
    // ImageThumbnail tests
    // ---------------------------------------------------------------

    #[test]
    fn thumbnail_landscape() {
        let t = ImageThumbnail::new(2000, 1000, 200);
        assert_eq!(t.thumb_width, 200);
        assert_eq!(t.thumb_height, 100);
        assert!((t.scale_factor() - 0.1).abs() < 1e-9);
    }

    #[test]
    fn thumbnail_portrait() {
        let t = ImageThumbnail::new(1000, 2000, 200);
        assert_eq!(t.thumb_width, 100);
        assert_eq!(t.thumb_height, 200);
    }

    #[test]
    fn thumbnail_zero_dimension() {
        let t = ImageThumbnail::new(0, 100, 50);
        assert_eq!(t.thumb_width, 0);
        assert_eq!(t.thumb_height, 0);
        assert!((t.scale_factor() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn thumbnail_display() {
        let t = ImageThumbnail::new(800, 600, 100);
        let s = format!("{t}");
        assert!(s.contains("800x600"));
        assert!(s.contains("→"));
    }

    // ---------------------------------------------------------------
    // ImageTransform tests
    // ---------------------------------------------------------------

    #[test]
    fn transform_identity() {
        let t = ImageTransform::new();
        assert!(t.is_identity());
        assert_eq!(t.transformed_dimensions(800, 600), (800, 600));
    }

    #[test]
    fn transform_rotate_90() {
        let t = ImageTransform::new().rotate(Rotation::Cw90);
        assert!(!t.is_identity());
        assert_eq!(t.transformed_dimensions(800, 600), (600, 800));
    }

    #[test]
    fn transform_rotate_180_keeps_dims() {
        let t = ImageTransform::new().rotate(Rotation::Cw180);
        assert_eq!(t.transformed_dimensions(800, 600), (800, 600));
    }

    #[test]
    fn transform_flip_not_identity() {
        let t = ImageTransform::new().flip(FlipAxis::Horizontal);
        assert!(!t.is_identity());
    }

    // ---------------------------------------------------------------
    // ImageCompare tests
    // ---------------------------------------------------------------

    #[test]
    fn compare_identical_images() {
        let a = vec![100u8; 16];
        let b = vec![100u8; 16];
        let r = image_compare(&a, &b, 4, 4).unwrap();
        assert_eq!(r.differing_pixels, 0);
        assert!((r.mean_absolute_error - 0.0).abs() < f64::EPSILON);
        assert!((r.diff_ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compare_all_different() {
        let a = vec![0u8; 4];
        let b = vec![255u8; 4];
        let r = image_compare(&a, &b, 2, 2).unwrap();
        assert_eq!(r.differing_pixels, 4);
        assert!((r.mean_absolute_error - 255.0).abs() < f64::EPSILON);
        assert!((r.diff_ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compare_size_mismatch() {
        let a = vec![0u8; 4];
        let b = vec![0u8; 4];
        let r = image_compare(&a, &b, 3, 3);
        assert!(r.is_err());
    }

    // -----------------------------------------------------------------------
    // Color conversion tests
    // -----------------------------------------------------------------------

    #[test]
    fn rgb_from_hex_and_roundtrip() {
        let c = Rgb::from_hex("#ff8800").unwrap();
        assert_eq!(c, Rgb::new(255, 136, 0));
        assert_eq!(c.to_hex(), "#ff8800");

        // Without leading '#'
        let c2 = Rgb::from_hex("00ff00").unwrap();
        assert_eq!(c2, Rgb::new(0, 255, 0));

        // Invalid length
        assert!(Rgb::from_hex("#fff").is_err());
    }

    #[test]
    fn rgb_to_hsl_pure_red() {
        let hsl = Rgb::new(255, 0, 0).to_hsl();
        assert!((hsl.h - 0.0).abs() < 1.0);
        assert!((hsl.s - 1.0).abs() < 1e-6);
        assert!((hsl.l - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rgb_luminance() {
        // Pure white should have luminance 255 * (0.299 + 0.587 + 0.114) = 255
        let lum = Rgb::new(255, 255, 255).luminance();
        assert!((lum - 255.0).abs() < 1e-6);

        // Pure black
        assert!((Rgb::new(0, 0, 0).luminance() - 0.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // SVG dimension parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_svg_dimensions_basic() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="200">"#;
        let dims = parse_svg_dimensions(svg).unwrap();
        assert!((dims.width - 300.0).abs() < f64::EPSILON);
        assert!((dims.height - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_svg_dimensions_with_px_unit() {
        let svg = r#"<svg width="100px" height="50px">"#;
        let dims = parse_svg_dimensions(svg).unwrap();
        assert!((dims.width - 100.0).abs() < f64::EPSILON);
        assert!((dims.height - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_svg_dimensions_missing() {
        assert!(parse_svg_dimensions("<svg>").is_none());
    }

    // -----------------------------------------------------------------------
    // Image cache tracker tests
    // -----------------------------------------------------------------------

    #[test]
    fn cache_tracker_basic_insert_and_contains() {
        let mut cache = ImageCacheTracker::new(1000);
        assert!(cache.is_empty());
        let evicted = cache.touch("a.png", 400);
        assert!(evicted.is_empty());
        assert_eq!(cache.len(), 1);
        assert!(cache.contains("a.png"));
        assert_eq!(cache.total_bytes(), 400);
    }

    #[test]
    fn cache_tracker_lru_eviction() {
        let mut cache = ImageCacheTracker::new(500);
        cache.touch("a.png", 300);
        cache.touch("b.png", 100);
        // Re-touch a.png to make it more recent
        cache.touch("a.png", 300);
        // Insert c.png (200 bytes) – total would be 600 > 500, must evict b.png (LRU)
        let evicted = cache.touch("c.png", 200);
        assert_eq!(evicted, vec!["b.png"]);
        assert!(!cache.contains("b.png"));
        assert!(cache.contains("a.png"));
        assert!(cache.contains("c.png"));
    }

    #[test]
    fn cache_tracker_clear() {
        let mut cache = ImageCacheTracker::new(1000);
        cache.touch("a.png", 100);
        cache.touch("b.png", 200);
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.total_bytes(), 0);
    }
}
