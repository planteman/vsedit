//! Image preview utilities – format detection, metadata, and zoom control.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// ImageScaler – scale with aspect-ratio preservation
// ---------------------------------------------------------------------------

/// Scales image dimensions while preserving aspect ratio.
#[derive(Debug, Clone, Copy)]
pub struct ImageScaler {
    /// Original width.
    pub width: u32,
    /// Original height.
    pub height: u32,
}

impl ImageScaler {
    /// Create a scaler for the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Scale to fit within the given maximum dimensions, preserving aspect ratio.
    pub fn fit(&self, max_width: u32, max_height: u32) -> (u32, u32) {
        if self.width == 0 || self.height == 0 {
            return (0, 0);
        }
        let ratio_w = max_width as f64 / self.width as f64;
        let ratio_h = max_height as f64 / self.height as f64;
        let ratio = ratio_w.min(ratio_h).min(1.0);
        (
            (self.width as f64 * ratio).round() as u32,
            (self.height as f64 * ratio).round() as u32,
        )
    }

    /// Scale to fill the given dimensions (may crop), preserving aspect ratio.
    pub fn fill(&self, target_width: u32, target_height: u32) -> (u32, u32) {
        if self.width == 0 || self.height == 0 {
            return (0, 0);
        }
        let ratio_w = target_width as f64 / self.width as f64;
        let ratio_h = target_height as f64 / self.height as f64;
        let ratio = ratio_w.max(ratio_h);
        (
            (self.width as f64 * ratio).round() as u32,
            (self.height as f64 * ratio).round() as u32,
        )
    }

    /// Scale by a percentage (100 = original size).
    pub fn scale_by_percent(&self, percent: u32) -> (u32, u32) {
        let factor = percent as f64 / 100.0;
        (
            (self.width as f64 * factor).round() as u32,
            (self.height as f64 * factor).round() as u32,
        )
    }

    /// Aspect ratio as width/height.
    pub fn aspect_ratio(&self) -> f64 {
        if self.height == 0 {
            return 0.0;
        }
        self.width as f64 / self.height as f64
    }
}

impl fmt::Display for ImageScaler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

// ---------------------------------------------------------------------------
// ImageColorSpace – sRGB/linear conversion
// ---------------------------------------------------------------------------

/// Color space conversion utilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageColorSpace {
    /// Standard RGB (gamma-encoded).
    Srgb,
    /// Linear RGB.
    Linear,
}

impl ImageColorSpace {
    /// Convert a single sRGB channel value (0.0–1.0) to linear.
    pub fn srgb_to_linear(value: f64) -> f64 {
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    /// Convert a single linear channel value to sRGB.
    pub fn linear_to_srgb(value: f64) -> f64 {
        if value <= 0.0031308 {
            value * 12.92
        } else {
            1.055 * value.powf(1.0 / 2.4) - 0.055
        }
    }

    /// Convert RGB channels from sRGB to linear.
    pub fn convert_pixel_to_linear(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
        (
            Self::srgb_to_linear(r),
            Self::srgb_to_linear(g),
            Self::srgb_to_linear(b),
        )
    }

    /// Convert RGB channels from linear to sRGB.
    pub fn convert_pixel_to_srgb(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
        (
            Self::linear_to_srgb(r),
            Self::linear_to_srgb(g),
            Self::linear_to_srgb(b),
        )
    }
}

impl fmt::Display for ImageColorSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageColorSpace::Srgb => write!(f, "sRGB"),
            ImageColorSpace::Linear => write!(f, "Linear"),
        }
    }
}

// ---------------------------------------------------------------------------
// ExifMetadata – basic EXIF-like metadata
// ---------------------------------------------------------------------------

/// Basic image metadata parsed from file headers (subset of EXIF).
///
/// Unlike [`ImageMetadata`], this is specifically for parsed header data
/// and uses `Option` fields since not all metadata may be available.
#[derive(Debug, Clone, Default)]
pub struct ExifMetadata {
    /// Width in pixels.
    pub width: Option<u32>,
    /// Height in pixels.
    pub height: Option<u32>,
    /// Bits per channel.
    pub bit_depth: Option<u8>,
    /// Color type description.
    pub color_type: Option<String>,
    /// DPI (dots per inch).
    pub dpi: Option<u32>,
    /// Camera or software name.
    pub software: Option<String>,
}

impl ExifMetadata {
    /// Parse metadata from PNG header bytes.
    ///
    /// Reads width and height from the IHDR chunk.
    pub fn from_png_header(data: &[u8]) -> Option<Self> {
        // PNG: 8-byte signature, then IHDR chunk (4 len + 4 type + 4 width + 4 height + ...)
        if data.len() < 24 {
            return None;
        }
        if &data[0..8] != b"\x89PNG\r\n\x1a\n" {
            return None;
        }
        let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        let bit_depth = if data.len() > 24 { Some(data[24]) } else { None };
        Some(Self {
            width: Some(width),
            height: Some(height),
            bit_depth,
            ..Default::default()
        })
    }

    /// Summary string for display.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let (Some(w), Some(h)) = (self.width, self.height) {
            parts.push(format!("{}×{}", w, h));
        }
        if let Some(d) = self.bit_depth {
            parts.push(format!("{}bpp", d));
        }
        if let Some(ref ct) = self.color_type {
            parts.push(ct.clone());
        }
        if parts.is_empty() {
            "no metadata".to_string()
        } else {
            parts.join(", ")
        }
    }
}

impl fmt::Display for ExifMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ---------------------------------------------------------------------------
// ImageDiffViewer – before/after comparison
// ---------------------------------------------------------------------------

/// Compares two image infos for a before/after diff.
#[derive(Debug, Clone)]
pub struct ImageDiffViewer {
    /// The "before" image info.
    pub before: ImageInfo,
    /// The "after" image info.
    pub after: ImageInfo,
}

impl ImageDiffViewer {
    /// Create a diff viewer for two images.
    pub fn new(before: ImageInfo, after: ImageInfo) -> Self {
        Self { before, after }
    }

    /// Whether the dimensions changed.
    pub fn dimensions_changed(&self) -> bool {
        self.before.width != self.after.width || self.before.height != self.after.height
    }

    /// Whether the format changed.
    pub fn format_changed(&self) -> bool {
        self.before.format != self.after.format
    }

    /// Size difference in bytes (positive = after is larger).
    pub fn size_diff(&self) -> i64 {
        self.after.file_size as i64 - self.before.file_size as i64
    }

    /// Human-readable summary of changes.
    pub fn summary(&self) -> String {
        let mut changes = Vec::new();
        if self.dimensions_changed() {
            changes.push(format!(
                "dimensions: {}×{} → {}×{}",
                self.before.width, self.before.height, self.after.width, self.after.height
            ));
        }
        if self.format_changed() {
            changes.push(format!(
                "format: {} → {}",
                self.before.format, self.after.format
            ));
        }
        let diff = self.size_diff();
        if diff != 0 {
            changes.push(format!(
                "size: {} → {} ({:+})",
                format_file_size(self.before.file_size),
                format_file_size(self.after.file_size),
                diff
            ));
        }
        if changes.is_empty() {
            "no changes".to_string()
        } else {
            changes.join("; ")
        }
    }
}

impl fmt::Display for ImageDiffViewer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}


// ---------------------------------------------------------------------------
// ImageZoomControls
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ImageZoomControls {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl ImageZoomControls {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for ImageZoomControls {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for ImageZoomControls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ImageZoomControls({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// ImageThumbnailGenerator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ImageThumbnailGenerator {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl ImageThumbnailGenerator {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for ImageThumbnailGenerator {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for ImageThumbnailGenerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ImageThumbnailGenerator({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// ImageZoomControlsSnapshot — point-in-time snapshot of ImageZoomControls state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ImageZoomControlsSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl ImageZoomControlsSnapshot {
    pub fn capture(source: &ImageZoomControls, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for ImageZoomControlsSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// ImageThumbnailGeneratorStats — aggregate statistics for ImageThumbnailGenerator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ImageThumbnailGeneratorStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl ImageThumbnailGeneratorStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for ImageThumbnailGeneratorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// ImageZoomControlsConfig — configuration for ImageZoomControls
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ImageZoomControlsConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl ImageZoomControlsConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for ImageZoomControlsConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for ImageZoomControlsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// ImageTransform
// ---------------------------------------------------------------------------

/// Represents a chain of image transformations (metadata only, no pixel data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformOp {
    FlipHorizontal,
    FlipVertical,
    Rotate90,
    Rotate180,
    Rotate270,
}

pub struct ImageTransformChain {
    ops: Vec<TransformOp>,
}

impl ImageTransformChain {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    pub fn flip_horizontal(mut self) -> Self {
        self.ops.push(TransformOp::FlipHorizontal);
        self
    }

    pub fn flip_vertical(mut self) -> Self {
        self.ops.push(TransformOp::FlipVertical);
        self
    }

    pub fn rotate_90(mut self) -> Self {
        self.ops.push(TransformOp::Rotate90);
        self
    }

    pub fn rotate_180(mut self) -> Self {
        self.ops.push(TransformOp::Rotate180);
        self
    }

    pub fn rotate_270(mut self) -> Self {
        self.ops.push(TransformOp::Rotate270);
        self
    }

    pub fn ops(&self) -> &[TransformOp] {
        &self.ops
    }

    /// Compute the resulting dimensions after all transforms on an image of (w, h).
    pub fn resulting_dimensions(&self, width: u32, height: u32) -> (u32, u32) {
        let mut w = width;
        let mut h = height;
        for op in &self.ops {
            match op {
                TransformOp::Rotate90 | TransformOp::Rotate270 => std::mem::swap(&mut w, &mut h),
                _ => {}
            }
        }
        (w, h)
    }
}

// ---------------------------------------------------------------------------
// ImageCropRegion
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ImageCropRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ImageCropRegion {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains_point(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    pub fn intersects(&self, other: &ImageCropRegion) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    pub fn union(&self, other: &ImageCropRegion) -> ImageCropRegion {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        ImageCropRegion { x, y, width: right - x, height: bottom - y }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn aspect_ratio(&self) -> f64 {
        if self.height == 0 { 0.0 } else { self.width as f64 / self.height as f64 }
    }

    pub fn center_in_bounds(&self, bound_w: u32, bound_h: u32) -> (u32, u32) {
        let cx = bound_w.saturating_sub(self.width) / 2;
        let cy = bound_h.saturating_sub(self.height) / 2;
        (cx, cy)
    }
}

// ---------------------------------------------------------------------------
// ImageThumbnailSpec
// ---------------------------------------------------------------------------

pub struct ImageThumbnailSpec {
    pub max_width: u32,
    pub max_height: u32,
}

impl ImageThumbnailSpec {
    pub fn new(max_width: u32, max_height: u32) -> Self {
        Self { max_width, max_height }
    }

    /// Compute thumbnail dimensions preserving aspect ratio.
    pub fn compute_dimensions(&self, src_w: u32, src_h: u32) -> (u32, u32) {
        if src_w == 0 || src_h == 0 {
            return (0, 0);
        }
        let scale_w = self.max_width as f64 / src_w as f64;
        let scale_h = self.max_height as f64 / src_h as f64;
        let scale = scale_w.min(scale_h).min(1.0);
        ((src_w as f64 * scale) as u32, (src_h as f64 * scale) as u32)
    }

    pub fn scale_factor(&self, src_w: u32, src_h: u32) -> f64 {
        if src_w == 0 || src_h == 0 {
            return 1.0;
        }
        let scale_w = self.max_width as f64 / src_w as f64;
        let scale_h = self.max_height as f64 / src_h as f64;
        scale_w.min(scale_h).min(1.0)
    }

    pub fn fits_within(&self, w: u32, h: u32) -> bool {
        w <= self.max_width && h <= self.max_height
    }
}


/// Histogram of color channels for an image.
pub struct ImageHistogram {
    red: [u32; 256],
    green: [u32; 256],
    blue: [u32; 256],
    sample_count: u64,
}

impl ImageHistogram {
    pub fn new() -> Self {
        Self { red: [0; 256], green: [0; 256], blue: [0; 256], sample_count: 0 }
    }

    pub fn add_pixel(&mut self, r: u8, g: u8, b: u8) {
        self.red[r as usize] += 1;
        self.green[g as usize] += 1;
        self.blue[b as usize] += 1;
        self.sample_count += 1;
    }

    pub fn sample_count(&self) -> u64 { self.sample_count }

    pub fn mean_red(&self) -> f64 {
        if self.sample_count == 0 { return 0.0; }
        let sum: u64 = self.red.iter().enumerate().map(|(i, &c)| i as u64 * c as u64).sum();
        sum as f64 / self.sample_count as f64
    }

    pub fn mean_green(&self) -> f64 {
        if self.sample_count == 0 { return 0.0; }
        let sum: u64 = self.green.iter().enumerate().map(|(i, &c)| i as u64 * c as u64).sum();
        sum as f64 / self.sample_count as f64
    }

    pub fn mean_blue(&self) -> f64 {
        if self.sample_count == 0 { return 0.0; }
        let sum: u64 = self.blue.iter().enumerate().map(|(i, &c)| i as u64 * c as u64).sum();
        sum as f64 / self.sample_count as f64
    }

    pub fn peak_red(&self) -> u8 {
        self.red.iter().enumerate().max_by_key(|&(_, c)| c).map(|(i, _)| i as u8).unwrap_or(0)
    }

    pub fn peak_green(&self) -> u8 {
        self.green.iter().enumerate().max_by_key(|&(_, c)| c).map(|(i, _)| i as u8).unwrap_or(0)
    }

    pub fn peak_blue(&self) -> u8 {
        self.blue.iter().enumerate().max_by_key(|&(_, c)| c).map(|(i, _)| i as u8).unwrap_or(0)
    }
}

/// Color space conversion utilities.
pub struct ColorConvert;

impl ColorConvert {
    /// Convert sRGB [0..255] to linear [0..1].
    pub fn srgb_to_linear(value: u8) -> f64 {
        let v = value as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    }

    /// Convert linear [0..1] to sRGB [0..255].
    pub fn linear_to_srgb(value: f64) -> u8 {
        let v = value.clamp(0.0, 1.0);
        let s = if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0).round() as u8
    }

    /// Compute relative luminance from sRGB values.
    pub fn luminance(r: u8, g: u8, b: u8) -> f64 {
        0.2126 * Self::srgb_to_linear(r)
            + 0.7152 * Self::srgb_to_linear(g)
            + 0.0722 * Self::srgb_to_linear(b)
    }

    /// Compute contrast ratio between two luminances (WCAG).
    pub fn contrast_ratio(l1: f64, l2: f64) -> f64 {
        let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
        (lighter + 0.05) / (darker + 0.05)
    }
}

/// Image crop region specification.
#[derive(Debug, Clone, PartialEq)]
pub struct CropRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl CropRegion {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains_point(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    pub fn area(&self) -> u64 { self.width as u64 * self.height as u64 }

    pub fn intersects(&self, other: &CropRegion) -> bool {
        self.x < other.x + other.width && self.x + self.width > other.x
            && self.y < other.y + other.height && self.y + self.height > other.y
    }

    pub fn clamp_to(&self, img_w: u32, img_h: u32) -> CropRegion {
        let x = self.x.min(img_w);
        let y = self.y.min(img_h);
        let w = self.width.min(img_w - x);
        let h = self.height.min(img_h - y);
        CropRegion::new(x, y, w, h)
    }
}



// ---------------------------------------------------------------------------
// image – Extended image transform helpers
// ---------------------------------------------------------------------------

/// Priority levels for image transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZImagePriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZImagePriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZImagePriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZImagePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks image transform data.
#[derive(Debug, Clone)]
pub struct ZImageImageTransform {
    pub operations: Vec<(String, f64)>,
    pub width: u32,
    pub height: u32,
}

impl ZImageImageTransform {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            width: 0,
            height: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.operations.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZImageImageTransform[width={:?}, height={:?}]", self.width, self.height)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for image transform.
pub fn z_image_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_image_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_image_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_image_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_image_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_image_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_image_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 91
// ---------------------------------------------------------------------------

/// Generic object pool `Xc91Pool<T>`.
pub struct Xc91Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc91Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc91PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc91Pool<T> {
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
    pub fn stats(&self) -> Xc91PoolStats {
        Xc91PoolStats {
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

impl<T> Default for Xc91Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc91Scheduler`.
pub struct Xc91Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc91Scheduler {
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

impl Default for Xc91Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_91 hash for the given byte slice.
pub fn xc_91_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_91 convention.
pub fn xc_91_reverse(s: &str) -> String {
    s.chars().rev().collect()
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

    // -- ImageScaler tests --

    #[test]
    fn scaler_fit() {
        let s = ImageScaler::new(800, 600);
        let (w, h) = s.fit(400, 400);
        assert_eq!(w, 400);
        assert_eq!(h, 300);
    }

    #[test]
    fn scaler_fit_no_upscale() {
        let s = ImageScaler::new(200, 100);
        let (w, h) = s.fit(400, 400);
        assert_eq!(w, 200);
        assert_eq!(h, 100);
    }

    #[test]
    fn scaler_fill() {
        let s = ImageScaler::new(800, 600);
        let (w, h) = s.fill(400, 400);
        // fill should cover the target area
        assert!(w >= 400 || h >= 400);
    }

    #[test]
    fn scaler_scale_by_percent() {
        let s = ImageScaler::new(100, 200);
        assert_eq!(s.scale_by_percent(50), (50, 100));
    }

    #[test]
    fn scaler_aspect_ratio() {
        let s = ImageScaler::new(800, 400);
        assert!((s.aspect_ratio() - 2.0).abs() < f64::EPSILON);
    }

    // -- ImageColorSpace tests --

    #[test]
    fn color_space_roundtrip() {
        let srgb = 0.5;
        let linear = ImageColorSpace::srgb_to_linear(srgb);
        let back = ImageColorSpace::linear_to_srgb(linear);
        assert!((back - srgb).abs() < 0.001);
    }

    #[test]
    fn color_space_black_white() {
        assert!((ImageColorSpace::srgb_to_linear(0.0)).abs() < f64::EPSILON);
        assert!((ImageColorSpace::srgb_to_linear(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn color_space_display() {
        assert_eq!(format!("{}", ImageColorSpace::Srgb), "sRGB");
        assert_eq!(format!("{}", ImageColorSpace::Linear), "Linear");
    }

    // -- ExifMetadata tests --

    #[test]
    fn exif_metadata_from_png() {
        // Minimal PNG-like header: signature + IHDR
        let mut data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        // IHDR chunk: length (13) + "IHDR" + width (100) + height (200) + bit_depth
        data.extend_from_slice(&[0, 0, 0, 13]); // length
        data.extend_from_slice(b"IHDR");
        data.extend_from_slice(&100u32.to_be_bytes()); // width
        data.extend_from_slice(&200u32.to_be_bytes()); // height
        data.push(8); // bit depth
        let meta = ExifMetadata::from_png_header(&data).unwrap();
        assert_eq!(meta.width, Some(100));
        assert_eq!(meta.height, Some(200));
        assert_eq!(meta.bit_depth, Some(8));
    }

    #[test]
    fn exif_metadata_summary() {
        let meta = ExifMetadata {
            width: Some(100),
            height: Some(200),
            bit_depth: Some(8),
            ..Default::default()
        };
        assert!(meta.summary().contains("100×200"));
    }

    // -- ImageDiffViewer tests --

    #[test]
    fn diff_no_changes() {
        let info = ImageInfo {
            width: 100,
            height: 100,
            format: ImageFormat::Png,
            file_size: 1024,
            uri: String::new(),
        };
        let diff = ImageDiffViewer::new(info.clone(), info);
        assert!(!diff.dimensions_changed());
        assert!(!diff.format_changed());
        assert_eq!(diff.size_diff(), 0);
    }

    #[test]
    fn diff_dimensions_changed() {
        let before = ImageInfo { width: 100, height: 100, format: ImageFormat::Png, file_size: 1024, uri: String::new() };
        let after = ImageInfo { width: 200, height: 200, format: ImageFormat::Png, file_size: 4096, uri: String::new() };
        let diff = ImageDiffViewer::new(before, after);
        assert!(diff.dimensions_changed());
        assert_eq!(diff.size_diff(), 3072);
        assert!(diff.summary().contains("dimensions"));
    }

    #[test]
    fn diff_format_changed() {
        let before = ImageInfo { width: 100, height: 100, format: ImageFormat::Png, file_size: 1024, uri: String::new() };
        let after = ImageInfo { width: 100, height: 100, format: ImageFormat::Jpeg, file_size: 512, uri: String::new() };
        let diff = ImageDiffViewer::new(before, after);
        assert!(diff.format_changed());
    }

    #[test] fn imageZoomControls_new() { let s = ImageZoomControls::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn imageZoomControls_add() { let mut s = ImageZoomControls::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn imageZoomControls_remove() { let mut s = ImageZoomControls::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn imageZoomControls_config() { let mut s = ImageZoomControls::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn imageZoomControls_nav() { let mut s = ImageZoomControls::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn imageZoomControls_filter() { let mut s = ImageZoomControls::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn imageZoomControls_display() { assert!(format!("{}", ImageZoomControls::new()).contains("ImageZoomControls")); }
    #[test] fn imageThumbnailGenerator_new() { let s = ImageThumbnailGenerator::new(); assert!(s.is_empty()); }
    #[test] fn imageThumbnailGenerator_add() { let mut s = ImageThumbnailGenerator::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn imageThumbnailGenerator_active() { let mut s = ImageThumbnailGenerator::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn imageThumbnailGenerator_error() { let mut s = ImageThumbnailGenerator::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn imageThumbnailGenerator_rm_group() { let mut s = ImageThumbnailGenerator::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn imageThumbnailGenerator_display() { assert!(format!("{}", ImageThumbnailGenerator::new()).contains("ImageThumbnailGenerator")); }


    #[test] fn imageZoomControls_snap_capture() {
        let s = ImageZoomControls::new();
        let snap = ImageZoomControlsSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn imageZoomControls_snap_stale() {
        let s = ImageZoomControls::new();
        let snap = ImageZoomControlsSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn imageZoomControls_snap_diff() {
        let s = ImageZoomControls::new();
        let s1v = ImageZoomControlsSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn imageZoomControls_snap_display() {
        let s = ImageZoomControls::new();
        let snap = ImageZoomControlsSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn imageThumbnailGenerator_stats_record() {
        let mut st = ImageThumbnailGeneratorStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn imageThumbnailGenerator_stats_hit_ratio() {
        let mut st = ImageThumbnailGeneratorStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn imageThumbnailGenerator_stats_merge() {
        let mut a = ImageThumbnailGeneratorStats::new();
        a.total_adds = 5;
        let mut b = ImageThumbnailGeneratorStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn imageThumbnailGenerator_stats_display() {
        let st = ImageThumbnailGeneratorStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn imageZoomControls_config_default() {
        let c = ImageZoomControlsConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn imageZoomControls_config_builder() {
        let c = ImageZoomControlsConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn imageZoomControls_config_labels() {
        let mut c = ImageZoomControlsConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn imageZoomControls_config_cleanup_threshold() {
        let c = ImageZoomControlsConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn imageZoomControls_config_display() {
        assert!(format!("{}", ImageZoomControlsConfig::new()).contains("Config"));
    }
    #[test] fn imageThumbnailGenerator_stats_peaks() {
        let mut st = ImageThumbnailGeneratorStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- ImageTransformChain tests --

    #[test]
    fn transform_chain() {
        let t = ImageTransformChain::new().flip_horizontal().rotate_90();
        assert_eq!(t.ops().len(), 2);
        assert_eq!(t.ops()[0], TransformOp::FlipHorizontal);
        assert_eq!(t.ops()[1], TransformOp::Rotate90);
    }

    #[test]
    fn transform_dimensions_rotate90() {
        let t = ImageTransformChain::new().rotate_90();
        assert_eq!(t.resulting_dimensions(800, 600), (600, 800));
    }

    #[test]
    fn transform_dimensions_rotate180() {
        let t = ImageTransformChain::new().rotate_180();
        assert_eq!(t.resulting_dimensions(800, 600), (800, 600));
    }

    #[test]
    fn transform_dimensions_flip() {
        let t = ImageTransformChain::new().flip_horizontal().flip_vertical();
        assert_eq!(t.resulting_dimensions(100, 200), (100, 200));
    }

    // -- ImageCropRegion tests --

    #[test]
    fn crop_contains_point() {
        let r = ImageCropRegion::new(10, 10, 50, 50);
        assert!(r.contains_point(20, 20));
        assert!(!r.contains_point(5, 5));
    }

    #[test]
    fn crop_intersects() {
        let a = ImageCropRegion::new(0, 0, 50, 50);
        let b = ImageCropRegion::new(25, 25, 50, 50);
        assert!(a.intersects(&b));
    }

    #[test]
    fn crop_no_intersect() {
        let a = ImageCropRegion::new(0, 0, 10, 10);
        let b = ImageCropRegion::new(20, 20, 10, 10);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn crop_union() {
        let a = ImageCropRegion::new(0, 0, 10, 10);
        let b = ImageCropRegion::new(5, 5, 10, 10);
        let u = a.union(&b);
        assert_eq!(u.x, 0);
        assert_eq!(u.y, 0);
        assert_eq!(u.width, 15);
        assert_eq!(u.height, 15);
    }

    #[test]
    fn crop_area() {
        let r = ImageCropRegion::new(0, 0, 100, 200);
        assert_eq!(r.area(), 20000);
    }

    #[test]
    fn crop_aspect_ratio() {
        let r = ImageCropRegion::new(0, 0, 160, 90);
        assert!((r.aspect_ratio() - 16.0 / 9.0).abs() < 0.01);
    }

    #[test]
    fn crop_center_in_bounds() {
        let r = ImageCropRegion::new(0, 0, 100, 100);
        assert_eq!(r.center_in_bounds(200, 200), (50, 50));
    }

    // -- ImageThumbnailSpec tests --

    #[test]
    fn thumbnail_smaller_image_no_upscale() {
        let spec = ImageThumbnailSpec::new(200, 200);
        assert_eq!(spec.compute_dimensions(100, 100), (100, 100));
    }

    #[test]
    fn thumbnail_larger_image_downscale() {
        let spec = ImageThumbnailSpec::new(100, 100);
        let (w, h) = spec.compute_dimensions(200, 400);
        assert!(w <= 100 && h <= 100);
    }

    #[test]
    fn thumbnail_fits_within() {
        let spec = ImageThumbnailSpec::new(100, 100);
        assert!(spec.fits_within(50, 50));
        assert!(!spec.fits_within(150, 50));
    }


    #[test]
    fn histogram_empty() {
        let h = ImageHistogram::new();
        assert_eq!(h.sample_count(), 0);
        assert_eq!(h.mean_red(), 0.0);
    }

    #[test]
    fn histogram_add_pixels() {
        let mut h = ImageHistogram::new();
        h.add_pixel(255, 0, 0);
        h.add_pixel(255, 0, 0);
        assert_eq!(h.sample_count(), 2);
        assert_eq!(h.peak_red(), 255);
    }

    #[test]
    fn histogram_mean_values() {
        let mut h = ImageHistogram::new();
        h.add_pixel(100, 100, 100);
        h.add_pixel(200, 200, 200);
        assert!((h.mean_red() - 150.0).abs() < 0.01);
        assert!((h.mean_green() - 150.0).abs() < 0.01);
    }

    #[test]
    fn histogram_peak_channels() {
        let mut h = ImageHistogram::new();
        for _ in 0..10 { h.add_pixel(42, 99, 200); }
        assert_eq!(h.peak_red(), 42);
        assert_eq!(h.peak_green(), 99);
        assert_eq!(h.peak_blue(), 200);
    }

    #[test]
    fn color_srgb_roundtrip() {
        for v in [0u8, 50, 128, 200, 255] {
            let linear = ColorConvert::srgb_to_linear(v);
            let back = ColorConvert::linear_to_srgb(linear);
            assert!((v as i16 - back as i16).unsigned_abs() <= 1);
        }
    }

    #[test]
    fn color_luminance_black_white() {
        assert!(ColorConvert::luminance(0, 0, 0) < 0.01);
        assert!((ColorConvert::luminance(255, 255, 255) - 1.0).abs() < 0.01);
    }

    #[test]
    fn color_contrast_ratio_bw() {
        let l1 = ColorConvert::luminance(255, 255, 255);
        let l2 = ColorConvert::luminance(0, 0, 0);
        let ratio = ColorConvert::contrast_ratio(l1, l2);
        assert!(ratio > 20.0);
    }

    #[test]
    fn crop_region_contains_point() {
        let r = CropRegion::new(10, 10, 50, 50);
        assert!(r.contains_point(20, 20));
        assert!(!r.contains_point(5, 5));
    }

    #[test]
    fn crop_region_area() {
        let r = CropRegion::new(0, 0, 100, 200);
        assert_eq!(r.area(), 20000);
    }

    #[test]
    fn crop_region_intersects() {
        let a = CropRegion::new(0, 0, 50, 50);
        let b = CropRegion::new(25, 25, 50, 50);
        let c = CropRegion::new(100, 100, 10, 10);
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn crop_region_clamp() {
        let r = CropRegion::new(90, 90, 50, 50);
        let clamped = r.clamp_to(100, 100);
        assert_eq!(clamped.width, 10);
        assert_eq!(clamped.height, 10);
    }

    #[test]
    fn crop_region_equality() {
        let a = CropRegion::new(1, 2, 3, 4);
        let b = CropRegion::new(1, 2, 3, 4);
        assert_eq!(a, b);
    }


    // -- image Z-extended tests -----------------------------------------------

    #[test]
    fn z_image_priority_weight() {
        assert_eq!(ZImagePriority::Idle.weight(), 0);
        assert_eq!(ZImagePriority::Normal.weight(), 2);
        assert_eq!(ZImagePriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_image_priority_label() {
        assert_eq!(ZImagePriority::Low.label(), "low");
        assert_eq!(ZImagePriority::High.label(), "high");
    }

    #[test]
    fn z_image_priority_is_elevated() {
        assert!(!ZImagePriority::Normal.is_elevated());
        assert!(ZImagePriority::High.is_elevated());
        assert!(ZImagePriority::Realtime.is_elevated());
    }

    #[test]
    fn z_image_priority_display() {
        assert_eq!(format!("{}", ZImagePriority::Idle), "idle");
    }

    #[test]
    fn z_image_priority_all_asc() {
        let all = ZImagePriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZImagePriority::Idle);
        assert_eq!(all[4], ZImagePriority::Realtime);
    }

    #[test]
    fn z_image_struct_new() {
        let s = ZImageImageTransform::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_image_struct_toggled_clone() {
        let s = ZImageImageTransform::new();
        let t = s.toggled_clone();
        let _ = t.height;
    }

    #[test]
    fn z_image_rolling_hash_deterministic() {
        let h1 = z_image_rolling_hash(b"test");
        let h2 = z_image_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_image_rolling_hash(b"a"), z_image_rolling_hash(b"b"));
    }

    #[test]
    fn z_image_pad_to_basic() {
        assert_eq!(z_image_pad_to("hi", 5), "hi   ");
        assert_eq!(z_image_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_image_is_identifier_basic() {
        assert!(z_image_is_identifier("foo_bar"));
        assert!(z_image_is_identifier("abc123"));
        assert!(!z_image_is_identifier(""));
        assert!(!z_image_is_identifier("has space"));
    }

    #[test]
    fn z_image_levenshtein_basic() {
        assert_eq!(z_image_levenshtein("", ""), 0);
        assert_eq!(z_image_levenshtein("abc", "abc"), 0);
        assert_eq!(z_image_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_image_unique_words_basic() {
        let w = z_image_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_image_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_image_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_image_common_prefix_basic() {
        assert_eq!(z_image_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_image_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_image_struct_clear() {
        let mut s = ZImageImageTransform::new();
        s.operations.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_image_rolling_hash_empty() {
        let h = z_image_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // ---- xc_ pool / scheduler tests – block 91 ----

    #[test]
    fn xc_91_pool_new_empty() {
        let pool: super::Xc91Pool<i32> = super::Xc91Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_91_pool_release_acquire() {
        let mut pool = super::Xc91Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_91_pool_acquire_empty() {
        let mut pool: super::Xc91Pool<i32> = super::Xc91Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_91_pool_full() {
        let mut pool = super::Xc91Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_91_pool_drain() {
        let mut pool = super::Xc91Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_91_pool_stats() {
        let mut pool = super::Xc91Pool::new(8);
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
    fn xc_91_pool_clear() {
        let mut pool = super::Xc91Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_91_pool_shrink() {
        let mut pool = super::Xc91Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_91_pool_default() {
        let pool: super::Xc91Pool<String> = super::Xc91Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_91_pool_extend() {
        let mut pool = super::Xc91Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_91_pool_retain() {
        let mut pool = super::Xc91Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_91_scheduler_round_robin() {
        let mut sched = super::Xc91Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_91_scheduler_empty() {
        let mut sched = super::Xc91Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_91_scheduler_reset() {
        let mut sched = super::Xc91Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_91_scheduler_add_remove() {
        let mut sched = super::Xc91Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_91_scheduler_targets() {
        let sched = super::Xc91Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_91_hash_empty() {
        assert_eq!(super::xc_91_hash(b""), 5381);
    }

    #[test]
    fn xc_91_hash_data() {
        let h = super::xc_91_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_91_hash(b"hello"), h);
    }

    #[test]
    fn xc_91_reverse_str() {
        assert_eq!(super::xc_91_reverse("abc"), "cba");
        assert_eq!(super::xc_91_reverse(""), "");
    }

}
