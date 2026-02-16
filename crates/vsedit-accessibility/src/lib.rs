//! Comprehensive accessibility support for vsedit, matching VS Code's
//! accessibility features: screen reader integration, high contrast mode,
//! keyboard navigation, audio cues, accessible editor content, and
//! color blind support.

use std::collections::HashMap;
use std::fmt;
use std::io::Write;

pub use vsedit_a11y::{
    AccessibilityConfig, AccessibilityError as A11yError, AccessibilityService,
    AccessibilitySupport, Announcement, AnnouncementPriority, AriaDescription, AriaRole,
    FocusTracker, KeyboardNavigation, ScreenReaderOptimized, Verbosity,
};
pub use vsedit_a11y_features::{
    AccessibilityFeaturesConfig, AccessibilityFeaturesConfigBuilder, AnnouncementQueue,
    HighContrastMode, ReducedMotionMode,
};

// ---------------------------------------------------------------------------
// 1. Screen reader support
// ---------------------------------------------------------------------------

/// Extended accessibility roles matching VS Code's widget semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityRole {
    TextBox,
    Button,
    Menu,
    MenuItem,
    TreeItem,
    Tab,
    StatusBar,
    Dialog,
    Alert,
    Progressbar,
}

impl fmt::Display for AccessibilityRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::TextBox => "textbox",
            Self::Button => "button",
            Self::Menu => "menu",
            Self::MenuItem => "menuitem",
            Self::TreeItem => "treeitem",
            Self::Tab => "tab",
            Self::StatusBar => "statusbar",
            Self::Dialog => "dialog",
            Self::Alert => "alert",
            Self::Progressbar => "progressbar",
        };
        write!(f, "{name}")
    }
}

/// Screen reader integration using terminal escape sequences.
///
/// Uses OSC (Operating System Command) escape sequences to communicate
/// with terminal-based screen readers, and queues announcements for
/// consumption by the TUI layer.
#[derive(Debug)]
pub struct ScreenReaderSupport {
    active: bool,
    labels: HashMap<String, String>,
    announcements: Vec<String>,
}

impl ScreenReaderSupport {
    pub fn new() -> Self {
        Self {
            active: false,
            labels: HashMap::new(),
            announcements: Vec::new(),
        }
    }

    /// Detect screen reader from environment variables.
    pub fn detect_from_env() -> bool {
        std::env::var("TERM_PROGRAM")
            .map(|v| v.contains("accessibility"))
            .unwrap_or(false)
            || std::env::var("ACCESSIBILITY_ENABLED")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false)
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn is_screen_reader_active(&self) -> bool {
        self.active
    }

    /// Send an announcement string. In a real terminal the output would go to
    /// an attached screen reader via OSC sequences. Here we queue the message
    /// for the TUI layer to consume.
    pub fn announce(&mut self, message: impl Into<String>) {
        let msg = message.into();
        if !msg.is_empty() {
            self.announcements.push(msg);
        }
    }

    /// Format an OSC escape sequence for screen reader output.
    pub fn format_osc_announcement(message: &str) -> String {
        // OSC 99 is used by some terminals for notifications
        format!("\x1b]99;{message}\x07")
    }

    /// Write an announcement directly to a writer (e.g. stdout) using
    /// terminal escape sequences.
    pub fn write_announcement<W: Write>(
        writer: &mut W,
        message: &str,
    ) -> std::io::Result<()> {
        write!(writer, "{}", Self::format_osc_announcement(message))
    }

    /// Set an accessible label for a UI element identified by `id`.
    pub fn set_aria_label(&mut self, id: impl Into<String>, label: impl Into<String>) {
        self.labels.insert(id.into(), label.into());
    }

    /// Get the accessible label for a UI element.
    pub fn get_aria_label(&self, id: &str) -> Option<&str> {
        self.labels.get(id).map(|s| s.as_str())
    }

    /// Remove an accessible label.
    pub fn remove_aria_label(&mut self, id: &str) -> bool {
        self.labels.remove(id).is_some()
    }

    /// Take all pending announcements.
    pub fn take_announcements(&mut self) -> Vec<String> {
        std::mem::take(&mut self.announcements)
    }

    /// Number of pending announcements.
    pub fn pending_count(&self) -> usize {
        self.announcements.len()
    }
}

impl Default for ScreenReaderSupport {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 2. High contrast mode
// ---------------------------------------------------------------------------

/// High contrast support for terminal UIs. Provides border and separator
/// styles together with contrast-aware color recommendations.
#[derive(Debug, Clone)]
pub struct HighContrastSupport {
    enabled: bool,
    mode: HighContrastMode,
}

impl HighContrastSupport {
    pub fn new() -> Self {
        Self {
            enabled: false,
            mode: HighContrastMode::None,
        }
    }

    /// Auto-detect high contrast from environment.
    pub fn detect() -> Self {
        let gtk = std::env::var("GTK_THEME").ok();
        let mode =
            AccessibilityFeaturesConfig::detect_high_contrast_from_env(gtk.as_deref());
        Self {
            enabled: mode != HighContrastMode::None,
            mode,
        }
    }

    pub fn is_high_contrast(&self) -> bool {
        self.enabled
    }

    pub fn mode(&self) -> HighContrastMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: HighContrastMode) {
        self.mode = mode;
        self.enabled = mode != HighContrastMode::None;
    }

    /// Return the bold border character to use in high contrast mode.
    pub fn border_char(&self) -> char {
        if self.enabled { '█' } else { '│' }
    }

    /// Return the thick separator string.
    pub fn separator(&self, width: usize) -> String {
        let ch = if self.enabled { '━' } else { '─' };
        std::iter::repeat(ch).take(width).collect()
    }

    /// Return foreground/background pair for maximum contrast.
    /// Returns `(fg, bg)` as ANSI color codes.
    pub fn contrast_colors(&self) -> (u8, u8) {
        match self.mode {
            HighContrastMode::Dark => (15, 0),   // white on black
            HighContrastMode::Light => (0, 15),   // black on white
            HighContrastMode::None => (7, 0),     // default
        }
    }

    /// Format text with underline for links/interactive elements in high
    /// contrast mode. Returns an ANSI-escaped string.
    pub fn underline_interactive(text: &str, high_contrast: bool) -> String {
        if high_contrast {
            format!("\x1b[4m{text}\x1b[24m")
        } else {
            text.to_string()
        }
    }
}

impl Default for HighContrastSupport {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 3. Keyboard navigation / Focus management
// ---------------------------------------------------------------------------

/// Predefined focus areas matching VS Code's layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusArea {
    Editor,
    Sidebar,
    Panel,
    ActivityBar,
    StatusBar,
    Menubar,
    TitleBar,
}

impl fmt::Display for FocusArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Editor => "Editor",
            Self::Sidebar => "Sidebar",
            Self::Panel => "Panel",
            Self::ActivityBar => "Activity Bar",
            Self::StatusBar => "Status Bar",
            Self::Menubar => "Menu Bar",
            Self::TitleBar => "Title Bar",
        };
        write!(f, "{name}")
    }
}

/// Defines the tab order of all UI components.
#[derive(Debug, Clone)]
pub struct FocusOrder {
    areas: Vec<FocusArea>,
}

impl FocusOrder {
    /// Default VS Code-style focus order.
    pub fn default_order() -> Self {
        Self {
            areas: vec![
                FocusArea::Editor,
                FocusArea::Sidebar,
                FocusArea::Panel,
                FocusArea::ActivityBar,
                FocusArea::StatusBar,
            ],
        }
    }

    pub fn custom(areas: Vec<FocusArea>) -> Self {
        Self { areas }
    }

    pub fn areas(&self) -> &[FocusArea] {
        &self.areas
    }

    pub fn len(&self) -> usize {
        self.areas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.areas.is_empty()
    }
}

impl Default for FocusOrder {
    fn default() -> Self {
        Self::default_order()
    }
}

/// Manages focus state across major UI areas with history.
#[derive(Debug, Clone)]
pub struct FocusManager {
    order: FocusOrder,
    current_index: usize,
    focus_history: Vec<FocusArea>,
    focus_ring_visible: bool,
}

impl FocusManager {
    pub fn new(order: FocusOrder) -> Self {
        Self {
            order,
            current_index: 0,
            focus_history: Vec::new(),
            focus_ring_visible: true,
        }
    }

    /// Current focused area.
    pub fn current_focus(&self) -> Option<FocusArea> {
        self.order.areas.get(self.current_index).copied()
    }

    /// Move focus to the next area (Tab).
    pub fn move_next(&mut self) -> Option<FocusArea> {
        if self.order.areas.is_empty() {
            return None;
        }
        if let Some(&area) = self.order.areas.get(self.current_index) {
            self.focus_history.push(area);
        }
        self.current_index = (self.current_index + 1) % self.order.areas.len();
        self.current_focus()
    }

    /// Move focus to the previous area (Shift+Tab).
    pub fn move_prev(&mut self) -> Option<FocusArea> {
        if self.order.areas.is_empty() {
            return None;
        }
        if let Some(&area) = self.order.areas.get(self.current_index) {
            self.focus_history.push(area);
        }
        if self.current_index == 0 {
            self.current_index = self.order.areas.len() - 1;
        } else {
            self.current_index -= 1;
        }
        self.current_focus()
    }

    /// Jump directly to a specific area.
    pub fn focus_area(&mut self, area: FocusArea) -> bool {
        if let Some(idx) = self.order.areas.iter().position(|&a| a == area) {
            if let Some(&cur) = self.order.areas.get(self.current_index) {
                self.focus_history.push(cur);
            }
            self.current_index = idx;
            true
        } else {
            false
        }
    }

    /// Return to the previous focus area from history.
    pub fn focus_back(&mut self) -> Option<FocusArea> {
        let prev = self.focus_history.pop()?;
        if let Some(idx) = self.order.areas.iter().position(|&a| a == prev) {
            self.current_index = idx;
        }
        Some(prev)
    }

    pub fn focus_history(&self) -> &[FocusArea] {
        &self.focus_history
    }

    /// Whether the focus ring (visual indicator) is visible.
    pub fn is_focus_ring_visible(&self) -> bool {
        self.focus_ring_visible
    }

    pub fn set_focus_ring_visible(&mut self, visible: bool) {
        self.focus_ring_visible = visible;
    }

    /// Render a focus indicator string around a label.
    pub fn focus_indicator(&self, label: &str, focused: bool) -> String {
        if focused && self.focus_ring_visible {
            format!("▶ {label} ◀")
        } else {
            label.to_string()
        }
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new(FocusOrder::default())
    }
}

// ---------------------------------------------------------------------------
// 4. Audio cues
// ---------------------------------------------------------------------------

/// Audio cue types matching VS Code's accessibility audio cues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioCue {
    Error,
    Warning,
    Breakpoint,
    TaskComplete,
    FoldingRange,
    LineHasError,
    LineHasWarning,
    TerminalBell,
    NotebookCellComplete,
    ChatRequestSent,
    Clear,
    Save,
}

impl fmt::Display for AudioCue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Breakpoint => "breakpoint",
            Self::TaskComplete => "task-complete",
            Self::FoldingRange => "folding-range",
            Self::LineHasError => "line-has-error",
            Self::LineHasWarning => "line-has-warning",
            Self::TerminalBell => "terminal-bell",
            Self::NotebookCellComplete => "notebook-cell-complete",
            Self::ChatRequestSent => "chat-request-sent",
            Self::Clear => "clear",
            Self::Save => "save",
        };
        write!(f, "{name}")
    }
}

/// Manages audio cue configuration and playback.
#[derive(Debug, Clone)]
pub struct AudioCueManager {
    enabled: HashMap<AudioCue, bool>,
    global_enabled: bool,
    played: Vec<AudioCue>,
}

impl AudioCueManager {
    pub fn new() -> Self {
        Self {
            enabled: HashMap::new(),
            global_enabled: true,
            played: Vec::new(),
        }
    }

    pub fn set_global_enabled(&mut self, enabled: bool) {
        self.global_enabled = enabled;
    }

    pub fn is_global_enabled(&self) -> bool {
        self.global_enabled
    }

    /// Enable or disable a specific cue type.
    pub fn set_cue_enabled(&mut self, cue: AudioCue, enabled: bool) {
        self.enabled.insert(cue, enabled);
    }

    /// Check if a specific cue type is enabled.
    pub fn is_cue_enabled(&self, cue: AudioCue) -> bool {
        self.global_enabled && *self.enabled.get(&cue).unwrap_or(&true)
    }

    /// Play an audio cue. Returns the terminal bell sequence if the cue is
    /// enabled, or `None` if suppressed.
    pub fn play_audio_cue(&mut self, cue: AudioCue) -> Option<&'static str> {
        if self.is_cue_enabled(cue) {
            self.played.push(cue);
            Some("\x07") // BEL character — terminal bell
        } else {
            None
        }
    }

    /// Write an audio cue directly to a writer.
    pub fn write_audio_cue<W: Write>(
        &mut self,
        writer: &mut W,
        cue: AudioCue,
    ) -> std::io::Result<bool> {
        if let Some(bell) = self.play_audio_cue(cue) {
            write!(writer, "{bell}")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Return the list of cues that have been played (for testing).
    pub fn played_cues(&self) -> &[AudioCue] {
        &self.played
    }

    /// Clear the played cues log.
    pub fn clear_played(&mut self) {
        self.played.clear();
    }
}

impl Default for AudioCueManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 5. Accessible editor content
// ---------------------------------------------------------------------------

/// Describes the accessible state of the editor for screen reader
/// consumption, mirroring VS Code's `IAccessibleViewContent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibleEditorContent {
    pub current_line_text: String,
    pub cursor_position_description: String,
    pub selection_description: String,
    pub diagnostics_description: String,
}

impl AccessibleEditorContent {
    pub fn new() -> Self {
        Self {
            current_line_text: String::new(),
            cursor_position_description: String::new(),
            selection_description: String::new(),
            diagnostics_description: String::new(),
        }
    }

    /// Build a description from editor state.
    pub fn from_editor_state(
        line_text: &str,
        line: usize,
        column: usize,
        selection: Option<(usize, usize, usize, usize)>,
        diagnostics: &[(&str, &str)],
    ) -> Self {
        let cursor_desc = format!("Line {line}, Column {column}");

        let selection_desc = match selection {
            Some((sl, sc, el, ec)) if sl == el => {
                format!("Selected columns {sc} to {ec} on line {sl}")
            }
            Some((sl, _sc, el, _ec)) => {
                format!("Selected from line {sl} to line {el}")
            }
            None => "No selection".to_string(),
        };

        let diag_desc = if diagnostics.is_empty() {
            "No problems".to_string()
        } else {
            let parts: Vec<String> = diagnostics
                .iter()
                .map(|(severity, msg)| format!("{severity}: {msg}"))
                .collect();
            parts.join("; ")
        };

        Self {
            current_line_text: line_text.to_string(),
            cursor_position_description: cursor_desc,
            selection_description: selection_desc,
            diagnostics_description: diag_desc,
        }
    }

    /// Generate a full screen-reader announcement for the current state.
    pub fn full_announcement(&self) -> String {
        format!(
            "{}. {}. {}. {}",
            self.cursor_position_description,
            self.current_line_text,
            self.selection_description,
            self.diagnostics_description
        )
    }

    /// Announce just the current line (used on cursor move).
    pub fn line_announcement(&self) -> String {
        format!(
            "{}: {}",
            self.cursor_position_description, self.current_line_text
        )
    }

    /// Announce diagnostics only.
    pub fn diagnostics_announcement(&self) -> String {
        self.diagnostics_description.clone()
    }
}

impl Default for AccessibleEditorContent {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AccessibleEditorContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.full_announcement())
    }
}

// ---------------------------------------------------------------------------
// 6. Color blind support
// ---------------------------------------------------------------------------

/// Color blind simulation modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorBlindMode {
    None,
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

impl fmt::Display for ColorBlindMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::None => "none",
            Self::Protanopia => "protanopia",
            Self::Deuteranopia => "deuteranopia",
            Self::Tritanopia => "tritanopia",
        };
        write!(f, "{name}")
    }
}

/// An RGB color tuple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// Provides alternative color palettes and pattern indicators for users
/// with color vision deficiency.
#[derive(Debug, Clone)]
pub struct ColorBlindSupport {
    mode: ColorBlindMode,
}

impl ColorBlindSupport {
    pub fn new(mode: ColorBlindMode) -> Self {
        Self { mode }
    }

    pub fn mode(&self) -> ColorBlindMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: ColorBlindMode) {
        self.mode = mode;
    }

    /// Get the adjusted error color for the current color blind mode.
    pub fn error_color(&self) -> Rgb {
        match self.mode {
            ColorBlindMode::None => Rgb(255, 0, 0),
            // Use orange tones that are distinguishable for red-green deficiency.
            ColorBlindMode::Protanopia | ColorBlindMode::Deuteranopia => Rgb(255, 128, 0),
            ColorBlindMode::Tritanopia => Rgb(255, 0, 0),
        }
    }

    /// Get the adjusted warning color.
    pub fn warning_color(&self) -> Rgb {
        match self.mode {
            ColorBlindMode::None => Rgb(255, 204, 0),
            ColorBlindMode::Protanopia | ColorBlindMode::Deuteranopia => Rgb(255, 255, 0),
            ColorBlindMode::Tritanopia => Rgb(255, 128, 0),
        }
    }

    /// Get the adjusted diff-added color.
    pub fn diff_added_color(&self) -> Rgb {
        match self.mode {
            ColorBlindMode::None => Rgb(0, 200, 0),
            // Blue tones for red-green deficiency.
            ColorBlindMode::Protanopia | ColorBlindMode::Deuteranopia => Rgb(0, 128, 255),
            ColorBlindMode::Tritanopia => Rgb(0, 200, 0),
        }
    }

    /// Get the adjusted diff-removed color.
    pub fn diff_removed_color(&self) -> Rgb {
        match self.mode {
            ColorBlindMode::None => Rgb(255, 0, 0),
            ColorBlindMode::Protanopia | ColorBlindMode::Deuteranopia => Rgb(255, 128, 0),
            ColorBlindMode::Tritanopia => Rgb(255, 0, 128),
        }
    }

    /// Return a pattern/shape prefix to use alongside color, providing a
    /// redundant visual cue.
    pub fn severity_indicator(&self, severity: &str) -> &'static str {
        match severity {
            "error" => "✖",   // cross
            "warning" => "⚠", // warning sign
            "info" => "ℹ",    // info
            "hint" => "💡",   // light bulb
            _ => "•",
        }
    }

    /// Return a diff marker character that doesn't rely solely on color.
    pub fn diff_marker(&self, kind: &str) -> &'static str {
        match kind {
            "added" => "+",
            "removed" => "-",
            "modified" => "~",
            _ => " ",
        }
    }
}

impl Default for ColorBlindSupport {
    fn default() -> Self {
        Self::new(ColorBlindMode::None)
    }
}

// ---------------------------------------------------------------------------
// Accessible buffer
// ---------------------------------------------------------------------------

/// Accumulates accessible text fragments for screen reader output.
///
/// This is used to build up a single screen reader announcement from
/// multiple sources (e.g. current line, diagnostics, cursor position).
#[derive(Debug, Clone)]
pub struct AccessibleBuffer {
    fragments: Vec<AccessibleFragment>,
}

#[derive(Debug, Clone)]
struct AccessibleFragment {
    text: String,
    role: AccessibilityRole,
    priority: u8,
}

impl AccessibleBuffer {
    pub fn new() -> Self {
        Self { fragments: Vec::new() }
    }

    /// Append a text fragment with a role and priority (0 = lowest).
    pub fn push(&mut self, text: impl Into<String>, role: AccessibilityRole, priority: u8) {
        self.fragments.push(AccessibleFragment {
            text: text.into(),
            role,
            priority,
        });
    }

    /// Append a plain text fragment with default role and priority.
    pub fn push_text(&mut self, text: impl Into<String>) {
        self.push(text, AccessibilityRole::TextBox, 0);
    }

    /// Append a status message at higher priority.
    pub fn push_status(&mut self, text: impl Into<String>) {
        self.push(text, AccessibilityRole::StatusBar, 5);
    }

    /// Render the buffer into a single string, sorted by priority (high first),
    /// with fragments separated by ". ".
    pub fn render(&self) -> String {
        let mut sorted: Vec<_> = self.fragments.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        let parts: Vec<&str> = sorted.iter().map(|f| f.text.as_str()).collect();
        parts.join(". ")
    }

    /// Return the number of accumulated fragments.
    pub fn len(&self) -> usize {
        self.fragments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }

    pub fn clear(&mut self) {
        self.fragments.clear();
    }

    /// Write the rendered output to a writer (e.g. for piping to a screen reader).
    pub fn write_to(&self, w: &mut dyn Write) -> std::io::Result<()> {
        write!(w, "{}", self.render())
    }

    /// Return fragments filtered by role.
    pub fn fragments_by_role(&self, role: AccessibilityRole) -> Vec<&str> {
        self.fragments
            .iter()
            .filter(|f| f.role == role)
            .map(|f| f.text.as_str())
            .collect()
    }
}

impl Default for AccessibleBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AccessibleBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

// ---------------------------------------------------------------------------
// Accessibility role lookup helpers
// ---------------------------------------------------------------------------

impl AccessibilityRole {
    /// Returns all role variants.
    pub fn all() -> &'static [AccessibilityRole] {
        &[
            AccessibilityRole::TextBox,
            AccessibilityRole::Button,
            AccessibilityRole::Menu,
            AccessibilityRole::MenuItem,
            AccessibilityRole::TreeItem,
            AccessibilityRole::Tab,
            AccessibilityRole::StatusBar,
            AccessibilityRole::Dialog,
            AccessibilityRole::Alert,
            AccessibilityRole::Progressbar,
        ]
    }

    /// Parse a role from its ARIA string name.
    pub fn from_aria_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "textbox" => Some(Self::TextBox),
            "button" => Some(Self::Button),
            "menu" => Some(Self::Menu),
            "menuitem" => Some(Self::MenuItem),
            "treeitem" => Some(Self::TreeItem),
            "tab" => Some(Self::Tab),
            "statusbar" | "status" => Some(Self::StatusBar),
            "dialog" => Some(Self::Dialog),
            "alert" => Some(Self::Alert),
            "progressbar" => Some(Self::Progressbar),
            _ => None,
        }
    }

    /// Returns the ARIA role string.
    pub fn aria_name(&self) -> &'static str {
        match self {
            Self::TextBox => "textbox",
            Self::Button => "button",
            Self::Menu => "menu",
            Self::MenuItem => "menuitem",
            Self::TreeItem => "treeitem",
            Self::Tab => "tab",
            Self::StatusBar => "statusbar",
            Self::Dialog => "dialog",
            Self::Alert => "alert",
            Self::Progressbar => "progressbar",
        }
    }

    /// Returns true if this role is interactive (can receive focus).
    pub fn is_interactive(&self) -> bool {
        matches!(self, Self::TextBox | Self::Button | Self::MenuItem | Self::Tab | Self::TreeItem)
    }
}

// ---------------------------------------------------------------------------
// Rgb helpers
// ---------------------------------------------------------------------------

impl Rgb {
    /// Creates a new Rgb from hex string like "#FF0000".
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Self(r, g, b))
    }

    /// Convert to hex string.
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }

    /// Compute relative luminance (0.0-1.0).
    pub fn luminance(&self) -> f64 {
        let r = self.0 as f64 / 255.0;
        let g = self.1 as f64 / 255.0;
        let b = self.2 as f64 / 255.0;
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// Returns true if this is a dark color.
    pub fn is_dark(&self) -> bool {
        self.luminance() < 0.5
    }

    /// Invert the color.
    pub fn invert(&self) -> Self {
        Self(255 - self.0, 255 - self.1, 255 - self.2)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rgb({}, {}, {})", self.0, self.1, self.2)
    }
}

impl Default for Rgb {
    fn default() -> Self {
        Self(0, 0, 0)
    }
}

impl From<(u8, u8, u8)> for Rgb {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self(r, g, b)
    }
}

// ---------------------------------------------------------------------------
// ColorBlindMode helpers
// ---------------------------------------------------------------------------

impl ColorBlindMode {
    /// Returns all color blind mode variants.
    pub fn all() -> &'static [ColorBlindMode] {
        &[
            ColorBlindMode::None,
            ColorBlindMode::Protanopia,
            ColorBlindMode::Deuteranopia,
            ColorBlindMode::Tritanopia,
        ]
    }

    /// Returns a human-readable description of the color blindness type.
    pub fn description(&self) -> &'static str {
        match self {
            ColorBlindMode::None => "Normal vision",
            ColorBlindMode::Protanopia => "Red-blind (missing L-cones)",
            ColorBlindMode::Deuteranopia => "Green-blind (missing M-cones)",
            ColorBlindMode::Tritanopia => "Blue-blind (missing S-cones)",
        }
    }
}

// ---------------------------------------------------------------------------
// AccessibilityAudit
// ---------------------------------------------------------------------------

/// Severity of an audit issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditSeverity {
    Pass,
    Warning,
    Error,
}

impl fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pass => write!(f, "pass"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A single issue found during an accessibility audit.
#[derive(Debug, Clone)]
pub struct AuditIssue {
    pub severity: AuditSeverity,
    pub message: String,
    pub widget_id: String,
}

impl fmt::Display for AuditIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.widget_id, self.message)
    }
}

/// Result of an accessibility audit.
#[derive(Debug, Clone, Default)]
pub struct AuditResult {
    pub issues: Vec<AuditIssue>,
}

impl AuditResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pass_count(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == AuditSeverity::Pass).count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == AuditSeverity::Warning).count()
    }

    pub fn error_count(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == AuditSeverity::Error).count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }

    pub fn is_clean(&self) -> bool {
        self.error_count() == 0 && self.warning_count() == 0
    }

    pub fn total(&self) -> usize {
        self.issues.len()
    }
}

impl fmt::Display for AuditResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AuditResult(pass={}, warn={}, error={})",
            self.pass_count(), self.warning_count(), self.error_count()
        )
    }
}

/// Audits a set of widgets for accessibility compliance.
#[derive(Debug)]
pub struct AccessibilityAudit {
    labels: HashMap<String, String>,
    keyboard_accessible: HashMap<String, bool>,
}

impl AccessibilityAudit {
    pub fn new() -> Self {
        Self {
            labels: HashMap::new(),
            keyboard_accessible: HashMap::new(),
        }
    }

    /// Register a widget with its label and keyboard accessibility.
    pub fn register_widget(&mut self, id: impl Into<String>, label: Option<String>, kb_accessible: bool) {
        let id = id.into();
        if let Some(label) = label {
            self.labels.insert(id.clone(), label);
        }
        self.keyboard_accessible.insert(id, kb_accessible);
    }

    /// Run the audit and return results.
    pub fn run(&self) -> AuditResult {
        let mut result = AuditResult::new();
        for (id, kb) in &self.keyboard_accessible {
            if !self.labels.contains_key(id) {
                result.issues.push(AuditIssue {
                    severity: AuditSeverity::Error,
                    message: "missing accessibility label".into(),
                    widget_id: id.clone(),
                });
            } else {
                result.issues.push(AuditIssue {
                    severity: AuditSeverity::Pass,
                    message: "has label".into(),
                    widget_id: id.clone(),
                });
            }
            if !kb {
                result.issues.push(AuditIssue {
                    severity: AuditSeverity::Warning,
                    message: "not keyboard accessible".into(),
                    widget_id: id.clone(),
                });
            }
        }
        result
    }
}

impl Default for AccessibilityAudit {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ContrastChecker
// ---------------------------------------------------------------------------

/// Checks color contrast ratios per WCAG guidelines.
#[derive(Debug)]
pub struct ContrastChecker;

impl ContrastChecker {
    /// Compute the contrast ratio between two relative luminances.
    /// Luminance values should be in 0.0..=1.0 range.
    pub fn check_ratio(lum1: f64, lum2: f64) -> f64 {
        let (lighter, darker) = if lum1 > lum2 { (lum1, lum2) } else { (lum2, lum1) };
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Returns the WCAG level for a given contrast ratio.
    pub fn wcag_level(ratio: f64) -> &'static str {
        if ratio >= 7.0 {
            "AAA"
        } else if ratio >= 4.5 {
            "AA"
        } else if ratio >= 3.0 {
            "AA-large"
        } else {
            "fail"
        }
    }

    /// Check contrast between two luminances and return the WCAG level.
    pub fn evaluate(lum1: f64, lum2: f64) -> &'static str {
        Self::wcag_level(Self::check_ratio(lum1, lum2))
    }

    /// Check contrast between two `Rgb` colors and return the ratio.
    pub fn check_rgb(a: &Rgb, b: &Rgb) -> f64 {
        Self::check_ratio(a.luminance(), b.luminance())
    }

    /// Evaluate WCAG level for two `Rgb` colors.
    pub fn evaluate_rgb(a: &Rgb, b: &Rgb) -> &'static str {
        Self::wcag_level(Self::check_rgb(a, b))
    }
}

// ---------------------------------------------------------------------------
// Accessible keyboard shortcuts
// ---------------------------------------------------------------------------

/// Describes an accessible keyboard shortcut with a description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibleShortcut {
    pub keys: String,
    pub action: String,
    pub category: String,
}

impl AccessibleShortcut {
    pub fn new(
        keys: impl Into<String>,
        action: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            keys: keys.into(),
            action: action.into(),
            category: category.into(),
        }
    }

    /// Format as a screen-reader-friendly announcement.
    pub fn announce(&self) -> String {
        format!("{}: press {} to {}", self.category, self.keys, self.action)
    }
}

impl fmt::Display for AccessibleShortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} → {}", self.category, self.keys, self.action)
    }
}

/// A registry of accessible keyboard shortcuts with lookup and filtering.
#[derive(Debug, Clone, Default)]
pub struct ShortcutRegistry {
    shortcuts: Vec<AccessibleShortcut>,
}

impl ShortcutRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, shortcut: AccessibleShortcut) {
        self.shortcuts.push(shortcut);
    }

    /// Return shortcuts filtered by category.
    pub fn by_category(&self, category: &str) -> Vec<&AccessibleShortcut> {
        self.shortcuts
            .iter()
            .filter(|s| s.category.eq_ignore_ascii_case(category))
            .collect()
    }

    /// Search shortcuts by action substring (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&AccessibleShortcut> {
        let q = query.to_lowercase();
        self.shortcuts
            .iter()
            .filter(|s| s.action.to_lowercase().contains(&q))
            .collect()
    }

    /// Return all unique categories.
    pub fn categories(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self.shortcuts.iter().map(|s| s.category.as_str()).collect();
        cats.sort_unstable();
        cats.dedup();
        cats
    }

    /// Generate a full announcement of all shortcuts, suitable for
    /// screen reader consumption.
    pub fn announce_all(&self) -> String {
        self.shortcuts
            .iter()
            .map(|s| s.announce())
            .collect::<Vec<_>>()
            .join(". ")
    }

    pub fn len(&self) -> usize {
        self.shortcuts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.shortcuts.is_empty()
    }
}

impl fmt::Display for ShortcutRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ShortcutRegistry({} shortcuts)", self.shortcuts.len())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Screen reader --

    #[test]
    fn screen_reader_default_inactive() {
        let sr = ScreenReaderSupport::new();
        assert!(!sr.is_screen_reader_active());
        assert_eq!(sr.pending_count(), 0);
    }

    #[test]
    fn screen_reader_announce_and_drain() {
        let mut sr = ScreenReaderSupport::new();
        sr.set_active(true);
        sr.announce("File opened");
        sr.announce("Cursor moved to line 5");
        assert_eq!(sr.pending_count(), 2);
        let msgs = sr.take_announcements();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0], "File opened");
        assert_eq!(msgs[1], "Cursor moved to line 5");
        assert_eq!(sr.pending_count(), 0);
    }

    #[test]
    fn screen_reader_empty_announce_ignored() {
        let mut sr = ScreenReaderSupport::new();
        sr.announce("");
        assert_eq!(sr.pending_count(), 0);
    }

    #[test]
    fn screen_reader_aria_labels() {
        let mut sr = ScreenReaderSupport::new();
        sr.set_aria_label("save-btn", "Save File");
        assert_eq!(sr.get_aria_label("save-btn"), Some("Save File"));
        assert_eq!(sr.get_aria_label("missing"), None);
        assert!(sr.remove_aria_label("save-btn"));
        assert!(!sr.remove_aria_label("save-btn"));
    }

    #[test]
    fn screen_reader_osc_format() {
        let osc = ScreenReaderSupport::format_osc_announcement("hello");
        assert!(osc.starts_with("\x1b]99;"));
        assert!(osc.ends_with("\x07"));
        assert!(osc.contains("hello"));
    }

    #[test]
    fn screen_reader_write_announcement() {
        let mut buf = Vec::new();
        ScreenReaderSupport::write_announcement(&mut buf, "test").unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("test"));
        assert!(output.contains("\x1b]99;"));
    }

    // -- High contrast --

    #[test]
    fn high_contrast_default_off() {
        let hc = HighContrastSupport::new();
        assert!(!hc.is_high_contrast());
        assert_eq!(hc.mode(), HighContrastMode::None);
    }

    #[test]
    fn high_contrast_set_mode() {
        let mut hc = HighContrastSupport::new();
        hc.set_mode(HighContrastMode::Dark);
        assert!(hc.is_high_contrast());
        assert_eq!(hc.mode(), HighContrastMode::Dark);
        assert_eq!(hc.border_char(), '█');
        let sep = hc.separator(5);
        assert_eq!(sep, "━━━━━");
        assert_eq!(hc.contrast_colors(), (15, 0));
    }

    #[test]
    fn high_contrast_normal_mode_chars() {
        let hc = HighContrastSupport::new();
        assert_eq!(hc.border_char(), '│');
        assert_eq!(hc.separator(3), "───");
    }

    #[test]
    fn high_contrast_underline_interactive() {
        let u = HighContrastSupport::underline_interactive("Link", true);
        assert!(u.contains("\x1b[4m"));
        assert!(u.contains("Link"));
        let plain = HighContrastSupport::underline_interactive("Link", false);
        assert_eq!(plain, "Link");
    }

    #[test]
    fn high_contrast_light_colors() {
        let mut hc = HighContrastSupport::new();
        hc.set_mode(HighContrastMode::Light);
        assert_eq!(hc.contrast_colors(), (0, 15));
    }

    // -- Focus management --

    #[test]
    fn focus_order_default() {
        let fo = FocusOrder::default();
        assert_eq!(fo.len(), 5);
        assert_eq!(fo.areas()[0], FocusArea::Editor);
    }

    #[test]
    fn focus_manager_navigation() {
        let mut fm = FocusManager::default();
        assert_eq!(fm.current_focus(), Some(FocusArea::Editor));

        let next = fm.move_next();
        assert_eq!(next, Some(FocusArea::Sidebar));

        let next = fm.move_next();
        assert_eq!(next, Some(FocusArea::Panel));

        let prev = fm.move_prev();
        assert_eq!(prev, Some(FocusArea::Sidebar));
    }

    #[test]
    fn focus_manager_wraps_around() {
        let mut fm = FocusManager::new(FocusOrder::custom(vec![
            FocusArea::Editor,
            FocusArea::Panel,
        ]));
        assert_eq!(fm.current_focus(), Some(FocusArea::Editor));
        fm.move_next(); // Panel
        let wrapped = fm.move_next();
        assert_eq!(wrapped, Some(FocusArea::Editor));
    }

    #[test]
    fn focus_manager_prev_wraps() {
        let mut fm = FocusManager::new(FocusOrder::custom(vec![
            FocusArea::Editor,
            FocusArea::Panel,
        ]));
        // At index 0, move prev should wrap to last
        let prev = fm.move_prev();
        assert_eq!(prev, Some(FocusArea::Panel));
    }

    #[test]
    fn focus_manager_direct_jump_and_history() {
        let mut fm = FocusManager::default();
        assert!(fm.focus_area(FocusArea::Panel));
        assert_eq!(fm.current_focus(), Some(FocusArea::Panel));

        let back = fm.focus_back();
        assert_eq!(back, Some(FocusArea::Editor));
    }

    #[test]
    fn focus_manager_indicator() {
        let fm = FocusManager::default();
        assert_eq!(fm.focus_indicator("Editor", true), "▶ Editor ◀");
        assert_eq!(fm.focus_indicator("Editor", false), "Editor");
    }

    // -- Audio cues --

    #[test]
    fn audio_cue_play_and_disable() {
        let mut acm = AudioCueManager::new();
        assert!(acm.is_cue_enabled(AudioCue::Error));
        let bell = acm.play_audio_cue(AudioCue::Error);
        assert_eq!(bell, Some("\x07"));
        assert_eq!(acm.played_cues().len(), 1);

        acm.set_cue_enabled(AudioCue::Warning, false);
        assert!(!acm.is_cue_enabled(AudioCue::Warning));
        assert_eq!(acm.play_audio_cue(AudioCue::Warning), None);
    }

    #[test]
    fn audio_cue_global_disable() {
        let mut acm = AudioCueManager::new();
        acm.set_global_enabled(false);
        assert_eq!(acm.play_audio_cue(AudioCue::Error), None);
        assert!(acm.played_cues().is_empty());
    }

    #[test]
    fn audio_cue_write_to_buffer() {
        let mut acm = AudioCueManager::new();
        let mut buf = Vec::new();
        let played = acm
            .write_audio_cue(&mut buf, AudioCue::TaskComplete)
            .unwrap();
        assert!(played);
        assert_eq!(buf, b"\x07");
    }

    // -- Accessible editor content --

    #[test]
    fn accessible_editor_content_from_state() {
        let content = AccessibleEditorContent::from_editor_state(
            "let x = 42;",
            10,
            5,
            Some((10, 5, 10, 10)),
            &[("error", "unused variable")],
        );
        assert_eq!(content.current_line_text, "let x = 42;");
        assert_eq!(content.cursor_position_description, "Line 10, Column 5");
        assert!(content.selection_description.contains("columns 5 to 10"));
        assert!(content.diagnostics_description.contains("unused variable"));
    }

    #[test]
    fn accessible_editor_no_selection_no_diagnostics() {
        let content =
            AccessibleEditorContent::from_editor_state("hello world", 1, 1, None, &[]);
        assert_eq!(content.selection_description, "No selection");
        assert_eq!(content.diagnostics_description, "No problems");
    }

    #[test]
    fn accessible_editor_multiline_selection() {
        let content = AccessibleEditorContent::from_editor_state(
            "line text",
            5,
            1,
            Some((3, 1, 7, 10)),
            &[],
        );
        assert!(content.selection_description.contains("line 3 to line 7"));
    }

    #[test]
    fn accessible_editor_line_announcement() {
        let content =
            AccessibleEditorContent::from_editor_state("fn main()", 1, 1, None, &[]);
        let ann = content.line_announcement();
        assert!(ann.contains("Line 1, Column 1"));
        assert!(ann.contains("fn main()"));
    }

    // -- Color blind support --

    #[test]
    fn color_blind_default_none() {
        let cbs = ColorBlindSupport::default();
        assert_eq!(cbs.mode(), ColorBlindMode::None);
        assert_eq!(cbs.error_color(), Rgb(255, 0, 0));
    }

    #[test]
    fn color_blind_protanopia_adjustments() {
        let cbs = ColorBlindSupport::new(ColorBlindMode::Protanopia);
        // Error should not be pure red for protanopia
        assert_ne!(cbs.error_color(), Rgb(255, 0, 0));
        // Diff added should use blue instead of green
        assert_eq!(cbs.diff_added_color(), Rgb(0, 128, 255));
    }

    #[test]
    fn color_blind_severity_indicators() {
        let cbs = ColorBlindSupport::default();
        assert_eq!(cbs.severity_indicator("error"), "✖");
        assert_eq!(cbs.severity_indicator("warning"), "⚠");
        assert_eq!(cbs.severity_indicator("info"), "ℹ");
    }

    #[test]
    fn color_blind_diff_markers() {
        let cbs = ColorBlindSupport::default();
        assert_eq!(cbs.diff_marker("added"), "+");
        assert_eq!(cbs.diff_marker("removed"), "-");
        assert_eq!(cbs.diff_marker("modified"), "~");
    }

    #[test]
    fn color_blind_mode_set() {
        let mut cbs = ColorBlindSupport::default();
        cbs.set_mode(ColorBlindMode::Tritanopia);
        assert_eq!(cbs.mode(), ColorBlindMode::Tritanopia);
        // Tritanopia warning uses orange
        assert_eq!(cbs.warning_color(), Rgb(255, 128, 0));
    }

    // -- Display impls --

    #[test]
    fn display_accessibility_role() {
        assert_eq!(format!("{}", AccessibilityRole::TextBox), "textbox");
        assert_eq!(format!("{}", AccessibilityRole::StatusBar), "statusbar");
        assert_eq!(format!("{}", AccessibilityRole::Progressbar), "progressbar");
    }

    #[test]
    fn display_audio_cue() {
        assert_eq!(format!("{}", AudioCue::Error), "error");
        assert_eq!(format!("{}", AudioCue::TaskComplete), "task-complete");
        assert_eq!(format!("{}", AudioCue::LineHasError), "line-has-error");
    }

    #[test]
    fn display_focus_area() {
        assert_eq!(format!("{}", FocusArea::Editor), "Editor");
        assert_eq!(format!("{}", FocusArea::ActivityBar), "Activity Bar");
    }

    #[test]
    fn display_color_blind_mode() {
        assert_eq!(format!("{}", ColorBlindMode::None), "none");
        assert_eq!(format!("{}", ColorBlindMode::Deuteranopia), "deuteranopia");
    }

    #[test]
    fn accessible_editor_content_display() {
        let content =
            AccessibleEditorContent::from_editor_state("let x = 1;", 3, 5, None, &[]);
        let display = format!("{content}");
        assert!(display.contains("Line 3"));
        assert!(display.contains("let x = 1;"));
    }

    // -- AccessibleBuffer --

    #[test]
    fn accessible_buffer_empty() {
        let buf = AccessibleBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.render(), "");
    }

    #[test]
    fn accessible_buffer_push_and_render() {
        let mut buf = AccessibleBuffer::new();
        buf.push_text("Line 5: let x = 1");
        buf.push_status("2 errors");
        let rendered = buf.render();
        assert!(rendered.starts_with("2 errors"));
        assert!(rendered.contains("let x = 1"));
    }

    #[test]
    fn accessible_buffer_priority_ordering() {
        let mut buf = AccessibleBuffer::new();
        buf.push("low", AccessibilityRole::TextBox, 0);
        buf.push("high", AccessibilityRole::StatusBar, 10);
        buf.push("mid", AccessibilityRole::Button, 5);
        assert!(buf.render().starts_with("high"));
    }

    #[test]
    fn accessible_buffer_clear() {
        let mut buf = AccessibleBuffer::new();
        buf.push_text("hello");
        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn accessible_buffer_write_to() {
        let mut buf = AccessibleBuffer::new();
        buf.push_text("test output");
        let mut output = Vec::new();
        buf.write_to(&mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "test output");
    }

    #[test]
    fn accessible_buffer_fragments_by_role() {
        let mut buf = AccessibleBuffer::new();
        buf.push_text("line content");
        buf.push_status("status msg");
        buf.push_text("more content");
        let text_frags = buf.fragments_by_role(AccessibilityRole::TextBox);
        assert_eq!(text_frags.len(), 2);
        let status_frags = buf.fragments_by_role(AccessibilityRole::StatusBar);
        assert_eq!(status_frags.len(), 1);
    }

    #[test]
    fn accessible_buffer_display() {
        let mut buf = AccessibleBuffer::new();
        buf.push_text("hello");
        assert_eq!(format!("{buf}"), "hello");
    }

    #[test]
    fn test_accessibility_role_all() {
        let roles = AccessibilityRole::all();
        assert_eq!(roles.len(), 10);
    }

    #[test]
    fn test_accessibility_role_from_aria_name() {
        assert_eq!(AccessibilityRole::from_aria_name("button"), Some(AccessibilityRole::Button));
        assert_eq!(AccessibilityRole::from_aria_name("TEXTBOX"), Some(AccessibilityRole::TextBox));
        assert_eq!(AccessibilityRole::from_aria_name("unknown"), None);
    }

    #[test]
    fn test_accessibility_role_aria_name() {
        assert_eq!(AccessibilityRole::Button.aria_name(), "button");
        assert_eq!(AccessibilityRole::TreeItem.aria_name(), "treeitem");
    }

    #[test]
    fn test_accessibility_role_is_interactive() {
        assert!(AccessibilityRole::Button.is_interactive());
        assert!(AccessibilityRole::TextBox.is_interactive());
        assert!(!AccessibilityRole::StatusBar.is_interactive());
        assert!(!AccessibilityRole::Alert.is_interactive());
    }

    #[test]
    fn test_rgb_from_hex() {
        let c = Rgb::from_hex("#FF8000").unwrap();
        assert_eq!(c.0, 255);
        assert_eq!(c.1, 128);
        assert_eq!(c.2, 0);
        assert!(Rgb::from_hex("bad").is_none());
    }

    #[test]
    fn test_rgb_to_hex() {
        let c = Rgb(255, 128, 0);
        assert_eq!(c.to_hex(), "#FF8000");
    }

    #[test]
    fn test_rgb_luminance_and_dark() {
        let white = Rgb(255, 255, 255);
        let black = Rgb(0, 0, 0);
        assert!(white.luminance() > 0.9);
        assert!(black.luminance() < 0.01);
        assert!(black.is_dark());
        assert!(!white.is_dark());
    }

    #[test]
    fn test_rgb_invert() {
        let c = Rgb(100, 200, 50);
        let inv = c.invert();
        assert_eq!(inv, Rgb(155, 55, 205));
    }

    #[test]
    fn test_rgb_display_and_from() {
        let c: Rgb = (10u8, 20u8, 30u8).into();
        assert_eq!(format!("{c}"), "rgb(10, 20, 30)");
        let d = Rgb::default();
        assert_eq!(d, Rgb(0, 0, 0));
    }

    #[test]
    fn test_color_blind_mode_all_and_desc() {
        let modes = ColorBlindMode::all();
        assert_eq!(modes.len(), 4);
        assert!(ColorBlindMode::Protanopia.description().contains("Red"));
        assert!(ColorBlindMode::None.description().contains("Normal"));
    }

    #[test]
    fn test_accessibility_audit_clean() {
        let mut audit = AccessibilityAudit::new();
        audit.register_widget("btn1", Some("Save".into()), true);
        audit.register_widget("btn2", Some("Cancel".into()), true);
        let result = audit.run();
        assert!(!result.has_errors());
        assert_eq!(result.pass_count(), 2);
        assert!(result.is_clean());
        assert!(format!("{result}").contains("pass=2"));
    }

    #[test]
    fn test_accessibility_audit_missing_label() {
        let mut audit = AccessibilityAudit::new();
        audit.register_widget("btn1", None, true);
        let result = audit.run();
        assert!(result.has_errors());
        assert_eq!(result.error_count(), 1);
        assert!(!result.is_clean());
        let issue_str = format!("{}", result.issues[0]);
        assert!(issue_str.contains("missing accessibility label"));
    }

    #[test]
    fn test_accessibility_audit_no_keyboard() {
        let mut audit = AccessibilityAudit::new();
        audit.register_widget("icon", Some("Logo".into()), false);
        let result = audit.run();
        assert_eq!(result.warning_count(), 1);
        assert!(!result.has_errors());
    }

    #[test]
    fn test_contrast_checker_ratio() {
        let ratio = ContrastChecker::check_ratio(1.0, 0.0);
        assert!((ratio - 21.0).abs() < 0.1);
        assert_eq!(ContrastChecker::wcag_level(ratio), "AAA");
    }

    #[test]
    fn test_contrast_checker_levels() {
        assert_eq!(ContrastChecker::wcag_level(8.0), "AAA");
        assert_eq!(ContrastChecker::wcag_level(5.0), "AA");
        assert_eq!(ContrastChecker::wcag_level(3.5), "AA-large");
        assert_eq!(ContrastChecker::wcag_level(2.0), "fail");
    }

    #[test]
    fn test_contrast_checker_evaluate() {
        assert_eq!(ContrastChecker::evaluate(1.0, 0.0), "AAA");
        assert_eq!(ContrastChecker::evaluate(0.5, 0.5), "fail");
    }

    // --- new tests ---

    #[test]
    fn contrast_checker_rgb_evaluation() {
        let white = Rgb(255, 255, 255);
        let black = Rgb(0, 0, 0);
        let ratio = ContrastChecker::check_rgb(&white, &black);
        assert!(ratio > 20.0);
        assert_eq!(ContrastChecker::evaluate_rgb(&white, &black), "AAA");
    }

    #[test]
    fn accessible_shortcut_creation_and_announce() {
        let sc = AccessibleShortcut::new("Ctrl+S", "save file", "Editor");
        assert_eq!(sc.keys, "Ctrl+S");
        let announcement = sc.announce();
        assert!(announcement.contains("Editor"));
        assert!(announcement.contains("Ctrl+S"));
        assert!(announcement.contains("save file"));
    }

    #[test]
    fn shortcut_registry_add_and_search() {
        let mut reg = ShortcutRegistry::new();
        reg.add(AccessibleShortcut::new("Ctrl+S", "save file", "Editor"));
        reg.add(AccessibleShortcut::new("Ctrl+P", "open file", "Editor"));
        reg.add(AccessibleShortcut::new("Ctrl+`", "toggle terminal", "Terminal"));
        assert_eq!(reg.len(), 3);
        let results = reg.search("save");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].keys, "Ctrl+S");
    }

    #[test]
    fn shortcut_registry_by_category() {
        let mut reg = ShortcutRegistry::new();
        reg.add(AccessibleShortcut::new("Ctrl+S", "save", "Editor"));
        reg.add(AccessibleShortcut::new("Ctrl+B", "toggle sidebar", "View"));
        reg.add(AccessibleShortcut::new("Ctrl+P", "open", "Editor"));
        let editor_shortcuts = reg.by_category("Editor");
        assert_eq!(editor_shortcuts.len(), 2);
        let view_shortcuts = reg.by_category("View");
        assert_eq!(view_shortcuts.len(), 1);
    }

    #[test]
    fn shortcut_registry_categories() {
        let mut reg = ShortcutRegistry::new();
        reg.add(AccessibleShortcut::new("a", "action a", "Editor"));
        reg.add(AccessibleShortcut::new("b", "action b", "Terminal"));
        reg.add(AccessibleShortcut::new("c", "action c", "Editor"));
        let cats = reg.categories();
        assert_eq!(cats, vec!["Editor", "Terminal"]);
    }

    #[test]
    fn shortcut_registry_announce_all() {
        let mut reg = ShortcutRegistry::new();
        reg.add(AccessibleShortcut::new("Ctrl+S", "save", "Editor"));
        reg.add(AccessibleShortcut::new("Ctrl+Q", "quit", "App"));
        let announcement = reg.announce_all();
        assert!(announcement.contains("save"));
        assert!(announcement.contains("quit"));
        assert!(announcement.contains(". ")); // separator
    }

    #[test]
    fn shortcut_display() {
        let sc = AccessibleShortcut::new("Ctrl+Z", "undo", "Edit");
        let text = format!("{}", sc);
        assert!(text.contains("Ctrl+Z"));
        assert!(text.contains("undo"));
        assert!(text.contains("Edit"));
    }
}
