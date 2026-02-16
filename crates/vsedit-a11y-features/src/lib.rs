//! Accessibility features detection.

use std::collections::VecDeque;

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
}
