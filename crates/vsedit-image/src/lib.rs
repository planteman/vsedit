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
}
