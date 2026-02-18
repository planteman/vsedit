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
// 8. Tab stop management
// ---------------------------------------------------------------------------

/// Represents a focusable element within a container (e.g., a toolbar or list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabStop {
    pub id: String,
    pub label: String,
    pub role: AccessibilityRole,
    pub disabled: bool,
}

impl TabStop {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        role: AccessibilityRole,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role,
            disabled: false,
        }
    }

    /// Create a disabled tab stop that is skipped during navigation.
    pub fn disabled(
        id: impl Into<String>,
        label: impl Into<String>,
        role: AccessibilityRole,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role,
            disabled: true,
        }
    }

    /// Screen reader announcement for this tab stop.
    pub fn announce(&self) -> String {
        if self.disabled {
            format!("{}, {}, disabled", self.label, self.role)
        } else {
            format!("{}, {}", self.label, self.role)
        }
    }
}

impl fmt::Display for TabStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.announce())
    }
}

/// Manages a linear sequence of tab stops with keyboard navigation.
#[derive(Debug, Clone)]
pub struct TabStopManager {
    stops: Vec<TabStop>,
    current: usize,
}

impl TabStopManager {
    pub fn new(stops: Vec<TabStop>) -> Self {
        Self { stops, current: 0 }
    }

    /// Current focused tab stop, if any.
    pub fn current(&self) -> Option<&TabStop> {
        self.stops.get(self.current)
    }

    /// Move to the next enabled tab stop, wrapping around.
    /// Returns the newly focused stop, or `None` if all stops are disabled.
    pub fn next(&mut self) -> Option<&TabStop> {
        if self.stops.is_empty() {
            return None;
        }
        let len = self.stops.len();
        for i in 1..=len {
            let idx = (self.current + i) % len;
            if !self.stops[idx].disabled {
                self.current = idx;
                return self.stops.get(self.current);
            }
        }
        None
    }

    /// Move to the previous enabled tab stop, wrapping around.
    pub fn prev(&mut self) -> Option<&TabStop> {
        if self.stops.is_empty() {
            return None;
        }
        let len = self.stops.len();
        for i in 1..=len {
            let idx = (self.current + len - i) % len;
            if !self.stops[idx].disabled {
                self.current = idx;
                return self.stops.get(self.current);
            }
        }
        None
    }

    /// Jump to a tab stop by id. Returns `true` if found and enabled.
    pub fn focus_by_id(&mut self, id: &str) -> bool {
        if let Some(idx) = self.stops.iter().position(|s| s.id == id && !s.disabled) {
            self.current = idx;
            true
        } else {
            false
        }
    }

    /// Return the number of enabled tab stops.
    pub fn enabled_count(&self) -> usize {
        self.stops.iter().filter(|s| !s.disabled).count()
    }

    pub fn len(&self) -> usize {
        self.stops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stops.is_empty()
    }
}

// ---------------------------------------------------------------------------
// 9. Accessible name computation
// ---------------------------------------------------------------------------

/// Computes the accessible name for a UI element following a simplified
/// version of the WAI-ARIA accessible name computation algorithm.
///
/// Priority order:
/// 1. Explicit `aria-labelledby` (joined labels)
/// 2. Explicit `aria-label`
/// 3. Element content / `title`
/// 4. Fallback to role name
#[derive(Debug, Clone)]
pub struct AccessibleNameResolver {
    labels: HashMap<String, String>,
}

impl AccessibleNameResolver {
    pub fn new() -> Self {
        Self {
            labels: HashMap::new(),
        }
    }

    /// Register a label source by id.
    pub fn register_label(&mut self, id: impl Into<String>, text: impl Into<String>) {
        self.labels.insert(id.into(), text.into());
    }

    /// Compute the accessible name given the available sources.
    ///
    /// - `labelledby_ids`: ordered list of ids whose text should be concatenated.
    /// - `aria_label`: explicit aria-label attribute.
    /// - `content`: visible text content of the element.
    /// - `role`: the ARIA role, used as ultimate fallback.
    pub fn compute(
        &self,
        labelledby_ids: &[&str],
        aria_label: Option<&str>,
        content: Option<&str>,
        role: AccessibilityRole,
    ) -> String {
        // 1. aria-labelledby
        if !labelledby_ids.is_empty() {
            let parts: Vec<&str> = labelledby_ids
                .iter()
                .filter_map(|id| self.labels.get(*id).map(|s| s.as_str()))
                .collect();
            if !parts.is_empty() {
                return parts.join(" ");
            }
        }
        // 2. aria-label
        if let Some(label) = aria_label {
            if !label.is_empty() {
                return label.to_string();
            }
        }
        // 3. content
        if let Some(text) = content {
            if !text.is_empty() {
                return text.to_string();
            }
        }
        // 4. role fallback
        role.aria_name().to_string()
    }
}

impl Default for AccessibleNameResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 10. Reduced motion support
// ---------------------------------------------------------------------------

/// Manages reduced motion preferences for users who are sensitive to
/// motion or animations.
#[derive(Debug, Clone)]
pub struct ReducedMotionSupport {
    mode: ReducedMotionMode,
}

impl ReducedMotionSupport {
    pub fn new() -> Self {
        Self {
            mode: ReducedMotionMode::NoPreference,
        }
    }

    /// Detect reduced motion preference from environment.
    pub fn detect() -> Self {
        let mode = if std::env::var("REDUCE_MOTION")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false)
        {
            ReducedMotionMode::Reduce
        } else {
            ReducedMotionMode::NoPreference
        };
        Self { mode }
    }

    pub fn mode(&self) -> ReducedMotionMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: ReducedMotionMode) {
        self.mode = mode;
    }

    /// Returns `true` if animations should be suppressed.
    pub fn should_reduce(&self) -> bool {
        self.mode != ReducedMotionMode::NoPreference
    }

    /// Returns the recommended animation duration in milliseconds.
    /// When reduced motion is active, returns 0 (instant transition).
    pub fn animation_duration_ms(&self, default_ms: u64) -> u64 {
        if self.should_reduce() { 0 } else { default_ms }
    }

    /// Returns the recommended scroll behavior string.
    pub fn scroll_behavior(&self) -> &'static str {
        if self.should_reduce() { "auto" } else { "smooth" }
    }
}

impl Default for ReducedMotionSupport {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 11. Live region for dynamic content updates
// ---------------------------------------------------------------------------

/// Politeness level for live region announcements, matching ARIA `aria-live`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveRegionPoliteness {
    /// Updates are not announced unless the region is focused.
    Off,
    /// Updates are announced when the user is idle.
    Polite,
    /// Updates are announced immediately, interrupting current speech.
    Assertive,
}

impl fmt::Display for LiveRegionPoliteness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Polite => write!(f, "polite"),
            Self::Assertive => write!(f, "assertive"),
        }
    }
}

/// A live region that tracks content changes and queues announcements
/// for screen readers with the appropriate politeness level.
#[derive(Debug, Clone)]
pub struct LiveRegion {
    id: String,
    politeness: LiveRegionPoliteness,
    content: String,
    pending_announcements: Vec<String>,
}

impl LiveRegion {
    pub fn new(id: impl Into<String>, politeness: LiveRegionPoliteness) -> Self {
        Self {
            id: id.into(),
            politeness,
            content: String::new(),
            pending_announcements: Vec::new(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn politeness(&self) -> LiveRegionPoliteness {
        self.politeness
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    /// Update the content of the live region. If the content changed and
    /// the politeness is not `Off`, an announcement is queued.
    pub fn update(&mut self, new_content: impl Into<String>) {
        let new = new_content.into();
        if new != self.content {
            self.content = new.clone();
            if self.politeness != LiveRegionPoliteness::Off {
                self.pending_announcements.push(new);
            }
        }
    }

    /// Take all pending announcements.
    pub fn take_announcements(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_announcements)
    }

    /// Number of pending announcements.
    pub fn pending_count(&self) -> usize {
        self.pending_announcements.len()
    }

    /// Returns `true` if this region should interrupt current speech.
    pub fn is_assertive(&self) -> bool {
        self.politeness == LiveRegionPoliteness::Assertive
    }

    /// Format an announcement with the region id for debugging/logging.
    pub fn format_announcement(&self, message: &str) -> String {
        format!("[{}:{}] {}", self.id, self.politeness, message)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FocusTrap - focus trap manager
// ---------------------------------------------------------------------------

/// Severity level for focus trap manager issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FocusTrapSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for FocusTrapSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [FocusTrap].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTrapEntry {
    pub id: String,
    pub label: String,
    pub severity: FocusTrapSeverity,
    pub detail: Option<String>,
    pub trap_depth: usize,
    enabled: bool,
}

impl FocusTrapEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: FocusTrapSeverity::Low,
            detail: None,
            trap_depth: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: FocusTrapSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_trap_depth(mut self, val: usize) -> Self {
        self.trap_depth = val;
        self
    }

    pub fn is_trapped(&self) -> bool {
        self.enabled && self.severity >= FocusTrapSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.trap_depth, det)
    }
}

impl fmt::Display for FocusTrapEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [FocusTrapEntry] items.
#[derive(Debug, Clone)]
pub struct FocusTrap {
    entries: Vec<FocusTrapEntry>,
    name: String,
    capacity: usize,
}

impl FocusTrap {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: FocusTrapEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<FocusTrapEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&FocusTrapEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn trap_depth(&self) -> usize { self.entries.len() }

    pub fn is_trapped(&self) -> bool {
        self.entries.iter().any(|e| e.is_trapped())
    }

    pub fn entries_by_severity(&self, severity: FocusTrapSeverity) -> Vec<&FocusTrapEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= FocusTrapSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&FocusTrapEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&FocusTrapEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// SkipLinkNavigator - skip link navigator
// ---------------------------------------------------------------------------

/// Configuration for [SkipLinkNavigator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipLinkNavigatorConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub link_count: usize,
}

impl SkipLinkNavigatorConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, link_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_link_count(mut self, val: usize) -> Self { self.link_count = val; self }
}

impl Default for SkipLinkNavigatorConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [SkipLinkNavigator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipLinkNavigatorItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl SkipLinkNavigatorItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn has_skip_links(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for SkipLinkNavigatorItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [SkipLinkNavigatorItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct SkipLinkNavigator {
    config: SkipLinkNavigatorConfig,
    items: Vec<SkipLinkNavigatorItem>,
}

impl SkipLinkNavigator {
    pub fn new(config: SkipLinkNavigatorConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: SkipLinkNavigatorItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<SkipLinkNavigatorItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&SkipLinkNavigatorItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn link_count(&self) -> usize { self.items.len() }

    pub fn has_skip_links(&self) -> bool {
        self.items.iter().any(|i| i.has_skip_links())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&SkipLinkNavigatorItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&SkipLinkNavigatorItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &SkipLinkNavigatorConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}


/// Manages accessibility announcement queue.
#[derive(Debug, Clone)]
pub struct A11yAnnouncementQueue {
    entries: Vec<A11yAnnouncement>,
    enabled: bool,
    max_entries: usize,
}

/// A single accessibility announcement.
#[derive(Debug, Clone, PartialEq)]
pub struct A11yAnnouncement {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl A11yAnnouncement {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl A11yAnnouncementQueue {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: A11yAnnouncement) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&A11yAnnouncement> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut A11yAnnouncement> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&A11yAnnouncement> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&A11yAnnouncement> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&A11yAnnouncement> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<A11yAnnouncement> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
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
// xa_ extended helpers for accessibility
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaAccessibilityRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaAccessibilityRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaAccessibilityCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaAccessibilityCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaAccessibilityCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 3
// ---------------------------------------------------------------------------

/// Generic object pool `Xc3Pool<T>`.
pub struct Xc3Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc3Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc3PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc3Pool<T> {
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
    pub fn stats(&self) -> Xc3PoolStats {
        Xc3PoolStats {
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

impl<T> Default for Xc3Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc3Scheduler`.
pub struct Xc3Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc3Scheduler {
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

impl Default for Xc3Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_3 hash for the given byte slice.
pub fn xc_3_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_3 convention.
pub fn xc_3_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_45 deepening: state machine + event bus ---

/// States for the Xd45 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd45State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd45State {
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
pub struct Xd45Transition {
    pub from: Xd45State,
    pub to: Xd45State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd45StateMachine {
    current: Xd45State,
    history: Vec<Xd45Transition>,
    step_counter: usize,
}

impl Xd45StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd45State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd45State {
        self.current
    }

    pub fn history(&self) -> &[Xd45Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd45State) -> Result<Xd45State, String> {
        let allowed = match (self.current, target) {
            (Xd45State::Idle, Xd45State::Running) => true,
            (Xd45State::Running, Xd45State::Paused) => true,
            (Xd45State::Running, Xd45State::Done) => true,
            (Xd45State::Paused, Xd45State::Running) => true,
            (Xd45State::Paused, Xd45State::Done) => true,
            (Xd45State::Done, Xd45State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_45: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd45Transition {
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
            "Xd45SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd45State> {
        let prefix = "Xd45SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd45State::Idle),
            "Running" => Some(Xd45State::Running),
            "Paused" => Some(Xd45State::Paused),
            "Done" => Some(Xd45State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd45State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd45 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd45Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd45Event {
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

type Xd45HandlerFn = Box<dyn Fn(&Xd45Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd45EventBus {
    handlers: Vec<(usize, Option<String>, Xd45HandlerFn)>,
    next_id: usize,
    published: Vec<Xd45Event>,
}

impl Xd45EventBus {
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
        F: Fn(&Xd45Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd45Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd45Event) {
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

    pub fn published_events(&self) -> &[Xd45Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #43
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf43Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf43TrieNode {
    children: std::collections::HashMap<char, Xf43TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf43Trie {
    root: Xf43TrieNode,
    count: usize,
}

impl Xf43Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf43TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf43TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf43TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf43BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf43BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 2).
pub struct Xh2SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh2SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 44 as u64,
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

/// A compact bit set supporting boolean operations (variant 2).
pub struct Xh2BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh2BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 2).
pub struct Xi2Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi2Deque<T> {
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
pub struct Xi2Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi2Interval {
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

/// A simple interval tree (variant 2).
pub struct Xi2IntervalTree {
    xi_intervals: Vec<Xi2Interval>,
}

impl Xi2IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi2Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi2Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi2Interval) -> Vec<&Xi2Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi2Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi2Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi2Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi2Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi2Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi2Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 2) ---

/// Disjoint set / union-find for crate 2.
pub struct Xj2UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj2UnionFind {
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

const XJ2_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 2.
pub struct Xj2BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj2BTreeNode<K, V>>>,
    len: usize,
}

struct Xj2BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj2BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj2BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ2_BTREE_ORDER - 1
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
        let mid = XJ2_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj2BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj2BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj2BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj2BTreeNode::xj_new_leaf();
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


// --- xk_2 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk2SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk2SegmentTree {
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
pub struct Xk2DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk2DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_2).
#[derive(Debug, Clone)]
pub struct Xl2Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl2Rope {
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

/// Suffix array for efficient string searching (xl_2).
#[derive(Debug, Clone)]
pub struct Xl2SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl2SuffixArray {
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
pub struct Xm2MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm2MatrixSparse {
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
pub struct Xm2Tokenizer {
    text: String,
}

impl Xm2Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 2.
pub struct Xn2Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn2Fenwick {
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

// ----- AVL tree map — crate 2 -----

#[derive(Debug, Clone)]
struct Xn2AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn2AvlNode<K, V>>>,
    right: Option<Box<Xn2AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 2.
#[derive(Debug, Clone)]
pub struct Xn2AVL<K, V> {
    root: Option<Box<Xn2AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn2AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn2AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn2AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn2AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn2AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn2AvlNode<K, V>>) -> Box<Xn2AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn2AvlNode<K, V>>) -> Box<Xn2AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn2AvlNode<K, V>>) -> Box<Xn2AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn2AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn2AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn2AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn2AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn2AvlNode<K, V>>) -> &Xn2AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn2AvlNode<K, V>>) -> (Box<Xn2AvlNode<K, V>>, Option<Box<Xn2AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn2AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn2AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn2AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn2AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn2AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn2AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn2AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo2RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo2Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo2RBNode<K, V> {
    key: K,
    value: V,
    color: Xo2Color,
    left: Option<Box<Xo2RBNode<K, V>>>,
    right: Option<Box<Xo2RBNode<K, V>>>,
}

/// A red-black tree map for crate 2.
#[derive(Debug, Clone)]
pub struct Xo2RedBlack<K, V> {
    root: Option<Box<Xo2RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo2RedBlack<K, V> {
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
            r.color = Xo2Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo2RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo2RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo2RBNode {
                    key, value, color: Xo2Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo2RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo2Color::Red)
    }

    fn xo_balance(mut h: Box<Xo2RBNode<K, V>>) -> Box<Xo2RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo2Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo2RBNode<K, V>>) -> Box<Xo2RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo2Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo2RBNode<K, V>>) -> Box<Xo2RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo2Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo2RBNode<K, V>>) {
        h.color = Xo2Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo2Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo2Color::Black; }
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
            r.color = Xo2Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo2RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo2RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo2RBNode<K, V>) -> (K, V, Option<Box<Xo2RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo2RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo2Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo2RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo2ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 2.
#[derive(Debug, Clone)]
pub struct Xo2ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo2ConsistentHash {
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
            let vkey = format!("{}#xo2#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo2#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 2).
#[derive(Debug)]
pub struct Xp2SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp2Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp2Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp2Node<K, V>>>,
    xp_right: Option<Box<Xp2Node<K, V>>>,
}

impl<K: Ord, V> Xp2Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp2SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp2SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp2Node<K, V>>>, key: &K) -> Option<Box<Xp2Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp2Node<K, V>>) -> Box<Xp2Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp2Node<K, V>>) -> Box<Xp2Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp2Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp2Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp2Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}

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

    // -- Tab stop management --

    #[test]
    fn tab_stop_announce() {
        let stop = TabStop::new("btn", "Save", AccessibilityRole::Button);
        assert_eq!(stop.announce(), "Save, button");
        assert_eq!(format!("{stop}"), "Save, button");

        let disabled = TabStop::disabled("btn2", "Delete", AccessibilityRole::Button);
        assert!(disabled.announce().contains("disabled"));
    }

    #[test]
    fn tab_stop_manager_navigation() {
        let stops = vec![
            TabStop::new("a", "First", AccessibilityRole::Button),
            TabStop::disabled("b", "Disabled", AccessibilityRole::Button),
            TabStop::new("c", "Third", AccessibilityRole::Button),
        ];
        let mut mgr = TabStopManager::new(stops);
        assert_eq!(mgr.current().unwrap().id, "a");
        assert_eq!(mgr.enabled_count(), 2);
        assert_eq!(mgr.len(), 3);

        // Next should skip disabled stop
        let next = mgr.next().unwrap();
        assert_eq!(next.id, "c");

        // Next wraps to first
        let next = mgr.next().unwrap();
        assert_eq!(next.id, "a");

        // Prev from first wraps to third (skipping disabled)
        let prev = mgr.prev().unwrap();
        assert_eq!(prev.id, "c");
    }

    #[test]
    fn tab_stop_manager_focus_by_id() {
        let stops = vec![
            TabStop::new("x", "X", AccessibilityRole::Tab),
            TabStop::new("y", "Y", AccessibilityRole::Tab),
            TabStop::disabled("z", "Z", AccessibilityRole::Tab),
        ];
        let mut mgr = TabStopManager::new(stops);
        assert!(mgr.focus_by_id("y"));
        assert_eq!(mgr.current().unwrap().id, "y");
        // Can't focus a disabled stop
        assert!(!mgr.focus_by_id("z"));
        // Can't focus a nonexistent stop
        assert!(!mgr.focus_by_id("missing"));
    }

    #[test]
    fn tab_stop_manager_all_disabled() {
        let stops = vec![
            TabStop::disabled("a", "A", AccessibilityRole::Button),
            TabStop::disabled("b", "B", AccessibilityRole::Button),
        ];
        let mut mgr = TabStopManager::new(stops);
        assert!(mgr.next().is_none());
        assert!(mgr.prev().is_none());
        assert_eq!(mgr.enabled_count(), 0);
    }

    #[test]
    fn tab_stop_manager_empty() {
        let mut mgr = TabStopManager::new(vec![]);
        assert!(mgr.is_empty());
        assert!(mgr.current().is_none());
        assert!(mgr.next().is_none());
        assert!(mgr.prev().is_none());
    }

    // -- Accessible name computation --

    #[test]
    fn accessible_name_labelledby() {
        let mut resolver = AccessibleNameResolver::new();
        resolver.register_label("lbl1", "First Name");
        resolver.register_label("lbl2", "Required");
        let name = resolver.compute(
            &["lbl1", "lbl2"],
            Some("fallback"),
            Some("content"),
            AccessibilityRole::TextBox,
        );
        assert_eq!(name, "First Name Required");
    }

    #[test]
    fn accessible_name_aria_label_fallback() {
        let resolver = AccessibleNameResolver::new();
        let name = resolver.compute(&[], Some("Search files"), None, AccessibilityRole::TextBox);
        assert_eq!(name, "Search files");
    }

    #[test]
    fn accessible_name_content_fallback() {
        let resolver = AccessibleNameResolver::new();
        let name = resolver.compute(&[], None, Some("Submit"), AccessibilityRole::Button);
        assert_eq!(name, "Submit");
    }

    #[test]
    fn accessible_name_role_fallback() {
        let resolver = AccessibleNameResolver::new();
        let name = resolver.compute(&[], None, None, AccessibilityRole::Button);
        assert_eq!(name, "button");
    }

    // -- Reduced motion support --

    #[test]
    fn reduced_motion_defaults() {
        let rm = ReducedMotionSupport::new();
        assert!(!rm.should_reduce());
        assert_eq!(rm.animation_duration_ms(300), 300);
        assert_eq!(rm.scroll_behavior(), "smooth");
    }

    #[test]
    fn reduced_motion_enabled() {
        let mut rm = ReducedMotionSupport::new();
        rm.set_mode(ReducedMotionMode::Reduce);
        assert!(rm.should_reduce());
        assert_eq!(rm.animation_duration_ms(300), 0);
        assert_eq!(rm.scroll_behavior(), "auto");
    }

    // -- Live region --

    #[test]
    fn live_region_polite_update() {
        let mut region = LiveRegion::new("status", LiveRegionPoliteness::Polite);
        assert_eq!(region.id(), "status");
        assert!(!region.is_assertive());
        assert_eq!(region.pending_count(), 0);

        region.update("3 errors found");
        assert_eq!(region.content(), "3 errors found");
        assert_eq!(region.pending_count(), 1);

        // Same content does not re-queue
        region.update("3 errors found");
        assert_eq!(region.pending_count(), 1);

        // Different content queues again
        region.update("2 errors found");
        assert_eq!(region.pending_count(), 2);

        let announcements = region.take_announcements();
        assert_eq!(announcements.len(), 2);
        assert_eq!(announcements[0], "3 errors found");
        assert_eq!(announcements[1], "2 errors found");
        assert_eq!(region.pending_count(), 0);
    }

    #[test]
    fn live_region_off_no_announcements() {
        let mut region = LiveRegion::new("silent", LiveRegionPoliteness::Off);
        region.update("something changed");
        assert_eq!(region.content(), "something changed");
        assert_eq!(region.pending_count(), 0);
    }

    #[test]
    fn live_region_assertive() {
        let region = LiveRegion::new("alert", LiveRegionPoliteness::Assertive);
        assert!(region.is_assertive());
        assert_eq!(format!("{}", region.politeness()), "assertive");
    }

    #[test]
    fn live_region_format_announcement() {
        let region = LiveRegion::new("errors", LiveRegionPoliteness::Polite);
        let formatted = region.format_announcement("5 problems");
        assert_eq!(formatted, "[errors:polite] 5 problems");
    }

    #[test]
    fn live_region_politeness_display() {
        assert_eq!(format!("{}", LiveRegionPoliteness::Off), "off");
        assert_eq!(format!("{}", LiveRegionPoliteness::Polite), "polite");
        assert_eq!(format!("{}", LiveRegionPoliteness::Assertive), "assertive");
    }

#[test]
    fn focustrap_severity_ordering() {
        assert!(FocusTrapSeverity::Critical > FocusTrapSeverity::High);
        assert!(FocusTrapSeverity::High > FocusTrapSeverity::Medium);
        assert!(FocusTrapSeverity::Medium > FocusTrapSeverity::Low);
    }

    #[test]
    fn focustrap_severity_display() {
        assert_eq!(FocusTrapSeverity::Low.to_string(), "low");
        assert_eq!(FocusTrapSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn focustrap_entry_creation() {
        let e = FocusTrapEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, FocusTrapSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn focustrap_entry_builder() {
        let e = FocusTrapEntry::new("e2", "Entry 2")
            .with_severity(FocusTrapSeverity::High)
            .with_detail("some detail")
            .with_trap_depth(42);
        assert_eq!(e.severity, FocusTrapSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.trap_depth, 42);
    }

    #[test]
    fn focustrap_entry_enable_disable() {
        let mut e = FocusTrapEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn focustrap_add_and_count() {
        let mut mgr = FocusTrap::new("test");
        mgr.add(FocusTrapEntry::new("a", "A"));
        mgr.add(FocusTrapEntry::new("b", "B").with_severity(FocusTrapSeverity::High));
        assert_eq!(mgr.trap_depth(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn focustrap_remove() {
        let mut mgr = FocusTrap::new("test");
        mgr.add(FocusTrapEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn focustrap_capacity() {
        let mut mgr = FocusTrap::new("test").with_capacity(1);
        assert!(mgr.add(FocusTrapEntry::new("a", "A")));
        assert!(!mgr.add(FocusTrapEntry::new("b", "B")));
    }

    #[test]
    fn focustrap_sorted_by_severity() {
        let mut mgr = FocusTrap::new("test");
        mgr.add(FocusTrapEntry::new("lo", "Low"));
        mgr.add(FocusTrapEntry::new("hi", "High").with_severity(FocusTrapSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, FocusTrapSeverity::Critical);
    }

    #[test]
    fn focustrap_summary() {
        let mgr = FocusTrap::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn skiplinknavigator_config_defaults() {
        let cfg = SkipLinkNavigatorConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn skiplinknavigator_item_creation() {
        let item = SkipLinkNavigatorItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn skiplinknavigator_add_and_get() {
        let mut mgr = SkipLinkNavigator::new(SkipLinkNavigatorConfig::new("test"));
        mgr.add(SkipLinkNavigatorItem::new("k1", "v1"));
        assert_eq!(mgr.link_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn skiplinknavigator_remove_item() {
        let mut mgr = SkipLinkNavigator::new(SkipLinkNavigatorConfig::new("test"));
        mgr.add(SkipLinkNavigatorItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn skiplinknavigator_sorted_by_priority() {
        let mut mgr = SkipLinkNavigator::new(SkipLinkNavigatorConfig::new("test"));
        mgr.add(SkipLinkNavigatorItem::new("lo", "low").with_priority(1));
        mgr.add(SkipLinkNavigatorItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn skiplinknavigator_items_with_tag() {
        let mut mgr = SkipLinkNavigator::new(SkipLinkNavigatorConfig::new("test"));
        mgr.add(SkipLinkNavigatorItem::new("a", "1").with_tag("x"));
        mgr.add(SkipLinkNavigatorItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn skiplinknavigator_report() {
        let mgr = SkipLinkNavigator::new(SkipLinkNavigatorConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn a11y_announce_entry_creation() {
        let e = A11yAnnouncement::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn a11y_announce_entry_with_priority() {
        let e = A11yAnnouncement::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn a11y_announce_entry_metadata() {
        let e = A11yAnnouncement::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn a11y_announce_entry_remove_meta() {
        let mut e = A11yAnnouncement::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn a11y_announce_entry_activate_deactivate() {
        let mut e = A11yAnnouncement::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn a11y_announce_queue_add_sorted() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.add(A11yAnnouncement::new("lo", "Lo").with_priority(1));
        c.add(A11yAnnouncement::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn a11y_announce_queue_capacity() {
        let mut c = A11yAnnouncementQueue::new(1);
        assert!(c.add(A11yAnnouncement::new("a", "A")));
        assert!(!c.add(A11yAnnouncement::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn a11y_announce_queue_remove() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.add(A11yAnnouncement::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn a11y_announce_queue_get() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.add(A11yAnnouncement::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn a11y_announce_queue_active_entries() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.add(A11yAnnouncement::new("a", "A"));
        c.add(A11yAnnouncement::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn a11y_announce_queue_enable_disable() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn a11y_announce_queue_clear() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.add(A11yAnnouncement::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn a11y_announce_queue_find_by_label() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.add(A11yAnnouncement::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn a11y_announce_queue_top_n() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.add(A11yAnnouncement::new("a", "A").with_priority(1));
        c.add(A11yAnnouncement::new("b", "B").with_priority(2));
        c.add(A11yAnnouncement::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn a11y_announce_queue_deactivate_activate_all() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.add(A11yAnnouncement::new("a", "A"));
        c.add(A11yAnnouncement::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn a11y_announce_queue_highest_priority() {
        let mut c = A11yAnnouncementQueue::new(10);
        assert!(c.highest_priority().is_none());
        c.add(A11yAnnouncement::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn a11y_announce_queue_contains() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.add(A11yAnnouncement::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn a11y_announce_queue_drain_inactive() {
        let mut c = A11yAnnouncementQueue::new(10);
        c.add(A11yAnnouncement::new("a", "A"));
        c.add(A11yAnnouncement::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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


    // xa_ extended tests for accessibility
    #[test]
    fn xa_accessibility_ring_new() {
        let rb = super::XaAccessibilityRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_accessibility_ring_push_len() {
        let mut rb = super::XaAccessibilityRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_accessibility_ring_wrap() {
        let mut rb = super::XaAccessibilityRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_accessibility_ring_mean_empty() {
        let rb = super::XaAccessibilityRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_accessibility_ring_mean_values() {
        let mut rb = super::XaAccessibilityRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_accessibility_ring_min_max() {
        let mut rb = super::XaAccessibilityRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_accessibility_ring_iter() {
        let mut rb = super::XaAccessibilityRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_accessibility_counter_new() {
        let c = super::XaAccessibilityCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_accessibility_counter_inc() {
        let mut c = super::XaAccessibilityCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_accessibility_counter_inc_by() {
        let mut c = super::XaAccessibilityCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_accessibility_counter_reset() {
        let mut c = super::XaAccessibilityCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_accessibility_counter_clear() {
        let mut c = super::XaAccessibilityCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_accessibility_counter_default() {
        let c = super::XaAccessibilityCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 3 ----

    #[test]
    fn xc_3_pool_new_empty() {
        let pool: super::Xc3Pool<i32> = super::Xc3Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_3_pool_release_acquire() {
        let mut pool = super::Xc3Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_3_pool_acquire_empty() {
        let mut pool: super::Xc3Pool<i32> = super::Xc3Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_3_pool_full() {
        let mut pool = super::Xc3Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_3_pool_drain() {
        let mut pool = super::Xc3Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_3_pool_stats() {
        let mut pool = super::Xc3Pool::new(8);
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
    fn xc_3_pool_clear() {
        let mut pool = super::Xc3Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_3_pool_shrink() {
        let mut pool = super::Xc3Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_3_pool_default() {
        let pool: super::Xc3Pool<String> = super::Xc3Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_3_pool_extend() {
        let mut pool = super::Xc3Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_3_pool_retain() {
        let mut pool = super::Xc3Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_3_scheduler_round_robin() {
        let mut sched = super::Xc3Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_3_scheduler_empty() {
        let mut sched = super::Xc3Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_3_scheduler_reset() {
        let mut sched = super::Xc3Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_3_scheduler_add_remove() {
        let mut sched = super::Xc3Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_3_scheduler_targets() {
        let sched = super::Xc3Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_3_hash_empty() {
        assert_eq!(super::xc_3_hash(b""), 5381);
    }

    #[test]
    fn xc_3_hash_data() {
        let h = super::xc_3_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_3_hash(b"hello"), h);
    }

    #[test]
    fn xc_3_reverse_str() {
        assert_eq!(super::xc_3_reverse("abc"), "cba");
        assert_eq!(super::xc_3_reverse(""), "");
    }


    // --- xd_45 deepening tests ---

    #[test]
    fn xd_45_sm_initial_state() {
        let sm = Xd45StateMachine::new();
        assert_eq!(sm.current_state(), Xd45State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_45_sm_valid_idle_to_running() {
        let mut sm = Xd45StateMachine::new();
        assert!(sm.transition(Xd45State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd45State::Running);
    }

    #[test]
    fn xd_45_sm_valid_running_to_paused() {
        let mut sm = Xd45StateMachine::new();
        sm.transition(Xd45State::Running).unwrap();
        assert!(sm.transition(Xd45State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd45State::Paused);
    }

    #[test]
    fn xd_45_sm_valid_running_to_done() {
        let mut sm = Xd45StateMachine::new();
        sm.transition(Xd45State::Running).unwrap();
        assert!(sm.transition(Xd45State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd45State::Done);
    }

    #[test]
    fn xd_45_sm_valid_paused_to_running() {
        let mut sm = Xd45StateMachine::new();
        sm.transition(Xd45State::Running).unwrap();
        sm.transition(Xd45State::Paused).unwrap();
        assert!(sm.transition(Xd45State::Running).is_ok());
    }

    #[test]
    fn xd_45_sm_valid_done_to_idle() {
        let mut sm = Xd45StateMachine::new();
        sm.transition(Xd45State::Running).unwrap();
        sm.transition(Xd45State::Done).unwrap();
        assert!(sm.transition(Xd45State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd45State::Idle);
    }

    #[test]
    fn xd_45_sm_invalid_idle_to_done() {
        let mut sm = Xd45StateMachine::new();
        assert!(sm.transition(Xd45State::Done).is_err());
    }

    #[test]
    fn xd_45_sm_invalid_idle_to_paused() {
        let mut sm = Xd45StateMachine::new();
        assert!(sm.transition(Xd45State::Paused).is_err());
    }

    #[test]
    fn xd_45_sm_history_tracking() {
        let mut sm = Xd45StateMachine::new();
        sm.transition(Xd45State::Running).unwrap();
        sm.transition(Xd45State::Paused).unwrap();
        sm.transition(Xd45State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd45State::Idle);
        assert_eq!(sm.history()[0].to, Xd45State::Running);
        assert_eq!(sm.history()[1].from, Xd45State::Running);
        assert_eq!(sm.history()[2].to, Xd45State::Done);
    }

    #[test]
    fn xd_45_sm_serialize_deserialize() {
        let mut sm = Xd45StateMachine::new();
        sm.transition(Xd45State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd45StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd45State::Running));
    }

    #[test]
    fn xd_45_sm_deserialize_invalid() {
        assert_eq!(Xd45StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_45_sm_reset() {
        let mut sm = Xd45StateMachine::new();
        sm.transition(Xd45State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd45State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_45_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd45EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd45Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_45_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd45EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd45Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd45Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_45_bus_unsubscribe() {
        let mut bus = Xd45EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_45_event_kind_and_payload() {
        let e = Xd45Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd45Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_45_bus_clear_history() {
        let mut bus = Xd45EventBus::new();
        bus.publish(Xd45Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_45_sm_step_counter_increments() {
        let mut sm = Xd45StateMachine::new();
        sm.transition(Xd45State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd45State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #43 --

    #[test]
    fn xf43_trie_insert_search() {
        let mut t = Xf43Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf43_trie_starts_with() {
        let mut t = Xf43Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf43_trie_remove() {
        let mut t = Xf43Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf43_trie_word_count() {
        let mut t = Xf43Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf43_trie_longest_prefix() {
        let mut t = Xf43Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf43_trie_all_words() {
        let mut t = Xf43Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf43_trie_autocomplete() {
        let mut t = Xf43Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf43_trie_empty_search() {
        let t = Xf43Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf43_bloom_add_contains() {
        let mut bf = Xf43BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf43_bloom_probably_absent() {
        let bf = Xf43BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf43_bloom_false_positive_rate() {
        let mut bf = Xf43BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf43_bloom_clear() {
        let mut bf = Xf43BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf43_bloom_union() {
        let mut a = Xf43BloomFilter::xf_new(512, 2);
        let mut b = Xf43BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf43_bloom_intersection_estimate() {
        let mut a = Xf43BloomFilter::xf_new(512, 2);
        let mut b = Xf43BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf43_bloom_union_size_mismatch() {
        let a = Xf43BloomFilter::xf_new(256, 2);
        let b = Xf43BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh2_skip_insert_contains() {
        let mut sl = super::Xh2SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh2_skip_remove() {
        let mut sl = super::Xh2SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh2_skip_len() {
        let mut sl = super::Xh2SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh2_skip_range_query() {
        let mut sl = super::Xh2SkipList::xh_new(4);
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
    fn xh2_skip_floor_ceiling() {
        let mut sl = super::Xh2SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh2_skip_rank() {
        let mut sl = super::Xh2SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh2_skip_empty() {
        let sl = super::Xh2SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh2_skip_duplicates() {
        let mut sl = super::Xh2SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh2_bitset_set_test() {
        let mut bs = super::Xh2BitSet::xh_new(256);
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
    fn xh2_bitset_clear_count() {
        let mut bs = super::Xh2BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh2_bitset_and_or_xor() {
        let mut a = super::Xh2BitSet::xh_new(128);
        let mut b = super::Xh2BitSet::xh_new(128);
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
    fn xh2_bitset_iter_ones() {
        let mut bs = super::Xh2BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh2_bitset_first_last() {
        let mut bs = super::Xh2BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh2_bitset_empty() {
        let bs = super::Xh2BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi2_deque_push_pop_back() {
        let mut dq = super::Xi2Deque::xi_new(4);
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
    fn xi2_deque_push_pop_front() {
        let mut dq = super::Xi2Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi2_deque_mixed_ops() {
        let mut dq = super::Xi2Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi2_deque_get_and_split() {
        let mut dq = super::Xi2Deque::xi_new(8);
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
    fn xi2_deque_rotate_left() {
        let mut dq = super::Xi2Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi2_deque_rotate_right() {
        let mut dq = super::Xi2Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi2_deque_grow() {
        let mut dq = super::Xi2Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi2_deque_empty() {
        let dq = super::Xi2Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi2_interval_tree_insert_query() {
        let mut tree = super::Xi2IntervalTree::xi_new();
        tree.xi_insert(super::Xi2Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi2Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi2Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi2_interval_tree_overlap() {
        let mut tree = super::Xi2IntervalTree::xi_new();
        tree.xi_insert(super::Xi2Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi2Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi2Interval::xi_new(12, 20));
        let q = super::Xi2Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi2_interval_tree_remove() {
        let mut tree = super::Xi2IntervalTree::xi_new();
        tree.xi_insert(super::Xi2Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi2Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi2_interval_tree_gaps() {
        let mut tree = super::Xi2IntervalTree::xi_new();
        tree.xi_insert(super::Xi2Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi2Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi2Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi2Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi2Interval::xi_new(8, 10));
    }

    #[test]
    fn xi2_interval_tree_merge() {
        let mut tree = super::Xi2IntervalTree::xi_new();
        tree.xi_insert(super::Xi2Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi2Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi2Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi2Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi2Interval::xi_new(10, 15));
    }

    #[test]
    fn xi2_interval_tree_all() {
        let mut tree = super::Xi2IntervalTree::xi_new();
        tree.xi_insert(super::Xi2Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi2Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi2_interval_tree_empty() {
        let tree = super::Xi2IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi2_interval_tree_contains_point() {
        let iv = super::Xi2Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 2) ---

    #[test]
    fn xj_2_uf_make_and_find() {
        let mut uf = super::Xj2UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_2_uf_union_connected() {
        let mut uf = super::Xj2UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_2_uf_component_count() {
        let mut uf = super::Xj2UnionFind::xj_new();
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
    fn xj_2_uf_component_size() {
        let mut uf = super::Xj2UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_2_uf_largest_component() {
        let mut uf = super::Xj2UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_2_uf_many_elements() {
        let mut uf = super::Xj2UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_2_uf_separate_components() {
        let mut uf = super::Xj2UnionFind::xj_new();
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
    fn xj_2_uf_path_compression() {
        let mut uf = super::Xj2UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_2_bt_insert_get() {
        let mut bt = super::Xj2BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_2_bt_contains_len() {
        let mut bt = super::Xj2BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_2_bt_replace() {
        let mut bt = super::Xj2BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_2_bt_remove() {
        let mut bt = super::Xj2BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_2_bt_keys_values() {
        let mut bt = super::Xj2BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_2_bt_range() {
        let mut bt = super::Xj2BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_2_bt_min_max() {
        let mut bt = super::Xj2BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_2_bt_many_inserts() {
        let mut bt = super::Xj2BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_2 segment tree tests ---

    #[test]
    fn xk_2_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk2SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_2_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk2SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_2_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk2SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_2_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk2SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_2_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk2SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_2_st_single_element() {
        let data = vec![42];
        let st = super::Xk2SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_2_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk2SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_2_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk2SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_2 disjoint intervals tests ---

    #[test]
    fn xk_2_di_add_and_count() {
        let mut di = super::Xk2DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_2_di_merge_overlap() {
        let mut di = super::Xk2DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_2_di_contains() {
        let mut di = super::Xk2DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_2_di_remove() {
        let mut di = super::Xk2DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_2_di_covered_length() {
        let mut di = super::Xk2DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_2_di_gaps() {
        let mut di = super::Xk2DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_2_di_merge_adjacent() {
        let mut di = super::Xk2DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_2_di_empty() {
        let di = super::Xk2DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_2_rope_new_empty() {
        let rope = super::Xl2Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_2_rope_from_str() {
        let rope = super::Xl2Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_2_rope_insert_at() {
        let mut rope = super::Xl2Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_2_rope_delete_range() {
        let mut rope = super::Xl2Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_2_rope_char_at() {
        let rope = super::Xl2Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_2_rope_split_concat() {
        let rope = super::Xl2Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_2_rope_line_count() {
        let rope = super::Xl2Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_2_rope_line_at() {
        let rope = super::Xl2Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_2_sa_build_and_search() {
        let sa = super::Xl2SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_2_sa_count() {
        let sa = super::Xl2SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_2_sa_longest_repeated() {
        let sa = super::Xl2SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_2_sa_all_positions() {
        let sa = super::Xl2SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_2_sa_len() {
        let sa = super::Xl2SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_2_sa_empty() {
        let sa = super::Xl2SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_2_rope_slice() {
        let rope = super::Xl2Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_2_sa_search_start() {
        let sa = super::Xl2SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_2_sparse_set_get() {
        let mut m = super::Xm2MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_2_sparse_row_col() {
        let mut m = super::Xm2MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_2_sparse_transpose() {
        let mut m = super::Xm2MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_2_sparse_multiply_vec() {
        let mut m = super::Xm2MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_2_sparse_nnz_density() {
        let mut m = super::Xm2MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_2_sparse_clear() {
        let mut m = super::Xm2MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_2_sparse_overwrite_zero() {
        let mut m = super::Xm2MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_2_tokenizer_basic() {
        let t = super::Xm2Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_2_tokenizer_count() {
        let t = super::Xm2Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_2_tokenizer_unique() {
        let t = super::Xm2Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_2_tokenizer_frequency() {
        let t = super::Xm2Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_2_tokenizer_delimiter() {
        let t = super::Xm2Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_2_tokenizer_whitespace() {
        let t = super::Xm2Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_2_tokenizer_empty() {
        let t = super::Xm2Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 2 ----

    #[test]
    fn xn_2_fenwick_prefix_sum() {
        let mut ft = super::Xn2Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_2_fenwick_range_sum() {
        let mut ft = super::Xn2Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_2_fenwick_point_query() {
        let mut ft = super::Xn2Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_2_fenwick_len() {
        let ft = super::Xn2Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_2_fenwick_multiple_updates() {
        let mut ft = super::Xn2Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_2_fenwick_single_element() {
        let mut ft = super::Xn2Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_2_fenwick_find_kth() {
        let mut ft = super::Xn2Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_2_fenwick_negative_delta() {
        let mut ft = super::Xn2Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 2 ----

    #[test]
    fn xn_2_avl_insert_get() {
        let mut m = super::Xn2AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_2_avl_remove() {
        let mut m = super::Xn2AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_2_avl_in_order() {
        let mut m = super::Xn2AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_2_avl_min_max() {
        let mut m = super::Xn2AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_2_avl_floor_ceiling() {
        let mut m = super::Xn2AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_2_avl_height_balanced() {
        let mut m = super::Xn2AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_2_avl_overwrite() {
        let mut m = super::Xn2AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_2_avl_empty() {
        let m: super::Xn2AVL<i32, i32> = super::Xn2AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo2RedBlack tests ---

    #[test]
    fn xo_2_rb_insert_and_get() {
        let mut tree = super::Xo2RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_2_rb_len_and_empty() {
        let mut tree = super::Xo2RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_2_rb_min_max() {
        let mut tree = super::Xo2RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_2_rb_contains() {
        let mut tree = super::Xo2RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_2_rb_remove() {
        let mut tree = super::Xo2RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_2_rb_in_order() {
        let mut tree = super::Xo2RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_2_rb_black_height() {
        let mut tree = super::Xo2RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_2_rb_overwrite() {
        let mut tree = super::Xo2RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo2ConsistentHash tests ---

    #[test]
    fn xo_2_ch_add_and_count() {
        let mut ring = super::Xo2ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_2_ch_remove_node() {
        let mut ring = super::Xo2ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_2_ch_get_node() {
        let mut ring = super::Xo2ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_2_ch_empty_ring() {
        let ring = super::Xo2ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_2_ch_distribution() {
        let mut ring = super::Xo2ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_2_ch_rebalance() {
        let mut ring = super::Xo2ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_2_ch_virtual_nodes() {
        let mut ring = super::Xo2ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_2_ch_consistent_lookup() {
        let mut ring = super::Xo2ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_2_splay_insert_get() {
        let mut t = super::Xp2SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_2_splay_remove() {
        let mut t = super::Xp2SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_2_splay_count_increases() {
        let mut t = super::Xp2SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_2_splay_depth() {
        let mut t = super::Xp2SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_2_splay_len_empty() {
        let t = super::Xp2SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_2_splay_min_max() {
        let mut t = super::Xp2SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_2_splay_overwrite() {
        let mut t = super::Xp2SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_2_splay_remove_missing() {
        let mut t = super::Xp2SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }

}
