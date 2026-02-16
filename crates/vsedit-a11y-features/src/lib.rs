//! Accessibility features detection.

use std::collections::VecDeque;
use std::fmt;

/// Whether the platform has a screen reader active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilitySupport {
    Unknown,
    Disabled,
    Enabled,
}

/// High-contrast mode variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighContrastMode {
    None,
    Light,
    Dark,
}

/// Reduced-motion preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReducedMotionMode {
    NoPreference,
    Reduce,
}

/// Priority for screen reader announcements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncementPriority {
    /// Polite – can be interrupted.
    Polite,
    /// Assertive – interrupts current speech.
    Assertive,
}

/// A queued announcement for a screen reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    pub message: String,
    pub priority: AnnouncementPriority,
}

/// Queue of screen reader announcements (FIFO).
pub struct AnnouncementQueue {
    queue: VecDeque<Announcement>,
    max_size: usize,
}

impl AnnouncementQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            max_size,
        }
    }

    pub fn push(&mut self, message: String, priority: AnnouncementPriority) {
        if self.queue.len() >= self.max_size {
            self.queue.pop_front();
        }
        self.queue.push_back(Announcement { message, priority });
    }

    pub fn pop(&mut self) -> Option<Announcement> {
        self.queue.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

/// Configuration for accessibility features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilityFeaturesConfig {
    pub high_contrast: HighContrastMode,
    pub reduced_motion: ReducedMotionMode,
    pub font_size_override: Option<u32>,
    pub cursor_blinking: bool,
    pub accessibility_support: AccessibilitySupport,
}

impl Default for AccessibilityFeaturesConfig {
    fn default() -> Self {
        Self {
            high_contrast: HighContrastMode::None,
            reduced_motion: ReducedMotionMode::NoPreference,
            font_size_override: None,
            cursor_blinking: true,
            accessibility_support: AccessibilitySupport::Unknown,
        }
    }
}

impl AccessibilityFeaturesConfig {
    /// Apply a high-contrast mode, disabling cursor blinking for better
    /// visibility when any high-contrast theme is active.
    pub fn apply_high_contrast(&mut self, mode: HighContrastMode) {
        self.high_contrast = mode;
        if mode != HighContrastMode::None {
            self.cursor_blinking = false;
        }
    }

    /// Apply a reduced-motion preference, disabling cursor blinking when
    /// motion reduction is requested.
    pub fn apply_reduced_motion(&mut self, mode: ReducedMotionMode) {
        self.reduced_motion = mode;
        if mode == ReducedMotionMode::Reduce {
            self.cursor_blinking = false;
        }
    }

    /// Detect high contrast from an environment variable (e.g. GTK_THEME).
    pub fn detect_high_contrast_from_env(env_value: Option<&str>) -> HighContrastMode {
        match env_value {
            Some(v) if v.contains("HighContrast") && v.contains("Inverse") => {
                HighContrastMode::Dark
            }
            Some(v) if v.contains("HighContrast") => HighContrastMode::Light,
            _ => HighContrastMode::None,
        }
    }

    /// Detect reduced motion preference from an environment variable.
    pub fn detect_reduced_motion_from_env(env_value: Option<&str>) -> ReducedMotionMode {
        match env_value {
            Some("1") | Some("true") | Some("reduce") => ReducedMotionMode::Reduce,
            _ => ReducedMotionMode::NoPreference,
        }
    }

    /// Returns true when a screen reader is known to be active.
    pub fn is_screen_reader_active(&self) -> bool {
        self.accessibility_support == AccessibilitySupport::Enabled
    }

    /// Returns the effective font size, using the override if set, otherwise
    /// falling back to the provided default.
    pub fn effective_font_size(&self, default: u32) -> u32 {
        self.font_size_override.unwrap_or(default)
    }

    /// Returns true when any accessibility accommodation is active (high
    /// contrast, reduced motion, or screen reader).
    pub fn has_any_accommodation(&self) -> bool {
        self.high_contrast != HighContrastMode::None
            || self.reduced_motion == ReducedMotionMode::Reduce
            || self.is_screen_reader_active()
    }

    /// Merge another config on top of this one.  Fields from `other` take
    /// precedence except `font_size_override` which is only overwritten when
    /// `other` supplies a `Some` value.
    pub fn merge(&mut self, other: &AccessibilityFeaturesConfig) {
        self.high_contrast = other.high_contrast;
        self.reduced_motion = other.reduced_motion;
        self.cursor_blinking = other.cursor_blinking;
        self.accessibility_support = other.accessibility_support;
        if other.font_size_override.is_some() {
            self.font_size_override = other.font_size_override;
        }
    }
}

impl fmt::Display for AccessibilityFeaturesConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "A11y[contrast={}, motion={}, font={}, cursor_blink={}, reader={}]",
            self.high_contrast,
            self.reduced_motion,
            match self.font_size_override {
                Some(s) => s.to_string(),
                None => "default".to_string(),
            },
            self.cursor_blinking,
            self.accessibility_support,
        )
    }
}

// ---------------------------------------------------------------------------
// Display impls for enums
// ---------------------------------------------------------------------------

impl fmt::Display for AccessibilitySupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Disabled => write!(f, "disabled"),
            Self::Enabled => write!(f, "enabled"),
        }
    }
}

impl fmt::Display for HighContrastMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Light => write!(f, "light"),
            Self::Dark => write!(f, "dark"),
        }
    }
}

impl fmt::Display for ReducedMotionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPreference => write!(f, "no-preference"),
            Self::Reduce => write!(f, "reduce"),
        }
    }
}

impl fmt::Display for AnnouncementPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Polite => write!(f, "polite"),
            Self::Assertive => write!(f, "assertive"),
        }
    }
}

impl fmt::Display for Announcement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.priority, self.message)
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when configuring accessibility features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessibilityError {
    /// Font size is outside the acceptable range.
    InvalidFontSize { size: u32, min: u32, max: u32 },
    /// Announcement message is empty.
    EmptyAnnouncement,
    /// Queue capacity must be at least 1.
    InvalidQueueCapacity,
}

impl fmt::Display for AccessibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFontSize { size, min, max } => {
                write!(f, "font size {size} is outside the valid range {min}..={max}")
            }
            Self::EmptyAnnouncement => write!(f, "announcement message must not be empty"),
            Self::InvalidQueueCapacity => write!(f, "queue capacity must be at least 1"),
        }
    }
}

impl std::error::Error for AccessibilityError {}

// ---------------------------------------------------------------------------
// Builder for AccessibilityFeaturesConfig
// ---------------------------------------------------------------------------

/// Builder for [`AccessibilityFeaturesConfig`] with validation.
#[derive(Debug, Clone)]
pub struct AccessibilityFeaturesConfigBuilder {
    config: AccessibilityFeaturesConfig,
    min_font_size: u32,
    max_font_size: u32,
}

impl Default for AccessibilityFeaturesConfigBuilder {
    fn default() -> Self {
        Self {
            config: AccessibilityFeaturesConfig::default(),
            min_font_size: 6,
            max_font_size: 72,
        }
    }
}

impl AccessibilityFeaturesConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn high_contrast(mut self, mode: HighContrastMode) -> Self {
        self.config.high_contrast = mode;
        self
    }

    pub fn reduced_motion(mut self, mode: ReducedMotionMode) -> Self {
        self.config.reduced_motion = mode;
        self
    }

    pub fn accessibility_support(mut self, support: AccessibilitySupport) -> Self {
        self.config.accessibility_support = support;
        self
    }

    pub fn cursor_blinking(mut self, enabled: bool) -> Self {
        self.config.cursor_blinking = enabled;
        self
    }

    pub fn font_size(mut self, size: u32) -> Self {
        self.config.font_size_override = Some(size);
        self
    }

    pub fn font_size_range(mut self, min: u32, max: u32) -> Self {
        self.min_font_size = min;
        self.max_font_size = max;
        self
    }

    /// Build the configuration, returning an error if validation fails.
    pub fn build(mut self) -> Result<AccessibilityFeaturesConfig, AccessibilityError> {
        if let Some(size) = self.config.font_size_override {
            if size < self.min_font_size || size > self.max_font_size {
                return Err(AccessibilityError::InvalidFontSize {
                    size,
                    min: self.min_font_size,
                    max: self.max_font_size,
                });
            }
        }
        // Automatically disable cursor blinking when accommodations require it.
        if self.config.high_contrast != HighContrastMode::None
            || self.config.reduced_motion == ReducedMotionMode::Reduce
        {
            self.config.cursor_blinking = false;
        }
        Ok(self.config)
    }
}

// ---------------------------------------------------------------------------
// Validated push for AnnouncementQueue
// ---------------------------------------------------------------------------

impl AnnouncementQueue {
    /// Create a queue with validation on the capacity.
    pub fn try_new(max_size: usize) -> Result<Self, AccessibilityError> {
        if max_size == 0 {
            return Err(AccessibilityError::InvalidQueueCapacity);
        }
        Ok(Self {
            queue: VecDeque::new(),
            max_size,
        })
    }

    /// Push an announcement with validation that the message is non-empty.
    pub fn try_push(
        &mut self,
        message: String,
        priority: AnnouncementPriority,
    ) -> Result<(), AccessibilityError> {
        if message.is_empty() {
            return Err(AccessibilityError::EmptyAnnouncement);
        }
        self.push(message, priority);
        Ok(())
    }

    /// Drain all assertive announcements, leaving polite ones in order.
    pub fn drain_assertive(&mut self) -> Vec<Announcement> {
        let mut assertive = Vec::new();
        let mut remaining = VecDeque::new();
        while let Some(a) = self.queue.pop_front() {
            if a.priority == AnnouncementPriority::Assertive {
                assertive.push(a);
            } else {
                remaining.push_back(a);
            }
        }
        self.queue = remaining;
        assertive
    }

    /// Returns the current max capacity of the queue.
    pub fn capacity(&self) -> usize {
        self.max_size
    }
}

impl fmt::Debug for AnnouncementQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnnouncementQueue")
            .field("len", &self.queue.len())
            .field("max_size", &self.max_size)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = AccessibilityFeaturesConfig::default();
        assert_eq!(cfg.high_contrast, HighContrastMode::None);
        assert_eq!(cfg.reduced_motion, ReducedMotionMode::NoPreference);
        assert!(cfg.font_size_override.is_none());
        assert!(cfg.cursor_blinking);
        assert_eq!(cfg.accessibility_support, AccessibilitySupport::Unknown);
    }

    #[test]
    fn apply_high_contrast_disables_cursor_blinking() {
        let mut cfg = AccessibilityFeaturesConfig::default();
        cfg.apply_high_contrast(HighContrastMode::Dark);
        assert_eq!(cfg.high_contrast, HighContrastMode::Dark);
        assert!(!cfg.cursor_blinking);

        cfg.apply_high_contrast(HighContrastMode::None);
        assert_eq!(cfg.high_contrast, HighContrastMode::None);
    }

    #[test]
    fn apply_reduced_motion_disables_cursor_blinking() {
        let mut cfg = AccessibilityFeaturesConfig::default();
        cfg.apply_reduced_motion(ReducedMotionMode::Reduce);
        assert_eq!(cfg.reduced_motion, ReducedMotionMode::Reduce);
        assert!(!cfg.cursor_blinking);
    }

    #[test]
    fn font_size_override() {
        let mut cfg = AccessibilityFeaturesConfig::default();
        assert!(cfg.font_size_override.is_none());
        cfg.font_size_override = Some(18);
        assert_eq!(cfg.font_size_override, Some(18));
    }

    #[test]
    fn announcement_queue_fifo() {
        let mut q = AnnouncementQueue::new(10);
        q.push("first".into(), AnnouncementPriority::Polite);
        q.push("second".into(), AnnouncementPriority::Assertive);
        assert_eq!(q.len(), 2);
        let a = q.pop().unwrap();
        assert_eq!(a.message, "first");
        assert_eq!(a.priority, AnnouncementPriority::Polite);
        let b = q.pop().unwrap();
        assert_eq!(b.message, "second");
        assert!(q.is_empty());
    }

    #[test]
    fn announcement_queue_overflow() {
        let mut q = AnnouncementQueue::new(2);
        q.push("a".into(), AnnouncementPriority::Polite);
        q.push("b".into(), AnnouncementPriority::Polite);
        q.push("c".into(), AnnouncementPriority::Polite);
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop().unwrap().message, "b");
    }

    #[test]
    fn detect_high_contrast_from_env() {
        assert_eq!(
            AccessibilityFeaturesConfig::detect_high_contrast_from_env(Some("HighContrast")),
            HighContrastMode::Light
        );
        assert_eq!(
            AccessibilityFeaturesConfig::detect_high_contrast_from_env(Some(
                "HighContrastInverse"
            )),
            HighContrastMode::Dark
        );
        assert_eq!(
            AccessibilityFeaturesConfig::detect_high_contrast_from_env(Some("Adwaita")),
            HighContrastMode::None
        );
        assert_eq!(
            AccessibilityFeaturesConfig::detect_high_contrast_from_env(None),
            HighContrastMode::None
        );
    }

    #[test]
    fn detect_reduced_motion_from_env() {
        assert_eq!(
            AccessibilityFeaturesConfig::detect_reduced_motion_from_env(Some("1")),
            ReducedMotionMode::Reduce
        );
        assert_eq!(
            AccessibilityFeaturesConfig::detect_reduced_motion_from_env(Some("reduce")),
            ReducedMotionMode::Reduce
        );
        assert_eq!(
            AccessibilityFeaturesConfig::detect_reduced_motion_from_env(None),
            ReducedMotionMode::NoPreference
        );
    }

    #[test]
    fn screen_reader_detection() {
        let mut cfg = AccessibilityFeaturesConfig::default();
        assert!(!cfg.is_screen_reader_active());
        cfg.accessibility_support = AccessibilitySupport::Enabled;
        assert!(cfg.is_screen_reader_active());
    }

    #[test]
    fn effective_font_size_uses_override() {
        let mut cfg = AccessibilityFeaturesConfig::default();
        assert_eq!(cfg.effective_font_size(14), 14);
        cfg.font_size_override = Some(20);
        assert_eq!(cfg.effective_font_size(14), 20);
    }

    #[test]
    fn has_any_accommodation_reports_correctly() {
        let cfg = AccessibilityFeaturesConfig::default();
        assert!(!cfg.has_any_accommodation());

        let mut hc = AccessibilityFeaturesConfig::default();
        hc.high_contrast = HighContrastMode::Light;
        assert!(hc.has_any_accommodation());

        let mut rm = AccessibilityFeaturesConfig::default();
        rm.reduced_motion = ReducedMotionMode::Reduce;
        assert!(rm.has_any_accommodation());

        let mut sr = AccessibilityFeaturesConfig::default();
        sr.accessibility_support = AccessibilitySupport::Enabled;
        assert!(sr.has_any_accommodation());
    }

    #[test]
    fn merge_preserves_font_size_when_other_is_none() {
        let mut base = AccessibilityFeaturesConfig::default();
        base.font_size_override = Some(16);

        let overlay = AccessibilityFeaturesConfig {
            high_contrast: HighContrastMode::Dark,
            font_size_override: None,
            ..AccessibilityFeaturesConfig::default()
        };
        base.merge(&overlay);
        assert_eq!(base.high_contrast, HighContrastMode::Dark);
        assert_eq!(base.font_size_override, Some(16));
    }

    #[test]
    fn merge_overwrites_font_size_when_other_is_some() {
        let mut base = AccessibilityFeaturesConfig::default();
        base.font_size_override = Some(16);

        let overlay = AccessibilityFeaturesConfig {
            font_size_override: Some(24),
            ..AccessibilityFeaturesConfig::default()
        };
        base.merge(&overlay);
        assert_eq!(base.font_size_override, Some(24));
    }

    #[test]
    fn display_impls_produce_expected_strings() {
        assert_eq!(format!("{}", AccessibilitySupport::Enabled), "enabled");
        assert_eq!(format!("{}", HighContrastMode::Dark), "dark");
        assert_eq!(format!("{}", ReducedMotionMode::Reduce), "reduce");
        assert_eq!(format!("{}", AnnouncementPriority::Assertive), "assertive");

        let ann = Announcement {
            message: "hello".into(),
            priority: AnnouncementPriority::Polite,
        };
        assert_eq!(format!("{ann}"), "[polite] hello");
    }

    #[test]
    fn config_display() {
        let cfg = AccessibilityFeaturesConfig::default();
        let s = format!("{cfg}");
        assert!(s.starts_with("A11y["));
        assert!(s.contains("contrast=none"));
    }

    #[test]
    fn builder_happy_path() {
        let cfg = AccessibilityFeaturesConfigBuilder::new()
            .high_contrast(HighContrastMode::Light)
            .font_size(18)
            .build()
            .unwrap();
        assert_eq!(cfg.high_contrast, HighContrastMode::Light);
        assert_eq!(cfg.font_size_override, Some(18));
        // Builder should auto-disable cursor blinking for high contrast.
        assert!(!cfg.cursor_blinking);
    }

    #[test]
    fn builder_rejects_invalid_font_size() {
        let err = AccessibilityFeaturesConfigBuilder::new()
            .font_size(200)
            .build()
            .unwrap_err();
        assert_eq!(
            err,
            AccessibilityError::InvalidFontSize {
                size: 200,
                min: 6,
                max: 72
            }
        );
        // Display impl should be informative.
        let msg = format!("{err}");
        assert!(msg.contains("200"));
    }

    #[test]
    fn builder_custom_font_range() {
        let result = AccessibilityFeaturesConfigBuilder::new()
            .font_size_range(10, 30)
            .font_size(8)
            .build();
        assert!(result.is_err());

        let ok = AccessibilityFeaturesConfigBuilder::new()
            .font_size_range(10, 30)
            .font_size(15)
            .build();
        assert!(ok.is_ok());
    }

    #[test]
    fn queue_try_new_rejects_zero() {
        let err = AnnouncementQueue::try_new(0).unwrap_err();
        assert_eq!(err, AccessibilityError::InvalidQueueCapacity);
    }

    #[test]
    fn queue_try_push_rejects_empty_message() {
        let mut q = AnnouncementQueue::try_new(5).unwrap();
        let err = q
            .try_push(String::new(), AnnouncementPriority::Polite)
            .unwrap_err();
        assert_eq!(err, AccessibilityError::EmptyAnnouncement);
    }

    #[test]
    fn queue_drain_assertive() {
        let mut q = AnnouncementQueue::new(10);
        q.push("info".into(), AnnouncementPriority::Polite);
        q.push("alert1".into(), AnnouncementPriority::Assertive);
        q.push("note".into(), AnnouncementPriority::Polite);
        q.push("alert2".into(), AnnouncementPriority::Assertive);

        let assertive = q.drain_assertive();
        assert_eq!(assertive.len(), 2);
        assert_eq!(assertive[0].message, "alert1");
        assert_eq!(assertive[1].message, "alert2");
        // Only polite announcements remain.
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop().unwrap().message, "info");
    }
}
