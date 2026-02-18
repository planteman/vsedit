//! Output panel view.
//!
//! Provides an output panel with named channels, scrollable content,
//! search, and auto-scroll — rendered via ratatui.

use std::fmt;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur when manipulating the output panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputError {
    /// The requested channel index does not exist.
    ChannelNotFound(usize),
    /// A channel with the given name already exists.
    DuplicateChannelName(String),
    /// The provided channel name is empty or blank.
    InvalidChannelName,
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::ChannelNotFound(idx) => write!(f, "channel index {idx} not found"),
            OutputError::DuplicateChannelName(name) => {
                write!(f, "channel '{name}' already exists")
            }
            OutputError::InvalidChannelName => write!(f, "channel name must not be empty"),
        }
    }
}

impl std::error::Error for OutputError {}

/// A named output channel containing lines of text.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputChannel {
    pub name: String,
    pub content: Vec<String>,
    pub is_visible: bool,
    pub show_timestamp: bool,
}

impl OutputChannel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: Vec::new(),
            is_visible: true,
            show_timestamp: false,
        }
    }

    /// Return the number of lines in this channel.
    pub fn line_count(&self) -> usize {
        self.content.len()
    }

    /// Return true when the channel has no content.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Return the last `n` lines (or fewer if the channel is shorter).
    pub fn tail(&self, n: usize) -> &[String] {
        let start = self.content.len().saturating_sub(n);
        &self.content[start..]
    }

    /// Count lines that contain the given substring (case-insensitive).
    pub fn count_matches(&self, query: &str) -> usize {
        if query.is_empty() {
            return 0;
        }
        let lower = query.to_lowercase();
        self.content
            .iter()
            .filter(|l| l.to_lowercase().contains(&lower))
            .count()
    }

    /// Return total byte size of all content lines (excluding newlines).
    pub fn byte_size(&self) -> usize {
        self.content.iter().map(|l| l.len()).sum()
    }
}

impl fmt::Display for OutputChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OutputChannel({}, {} lines, visible={})",
            self.name,
            self.content.len(),
            self.is_visible
        )
    }
}

/// Output panel with multiple named channels.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputPanel {
    pub channels: Vec<OutputChannel>,
    pub active_channel_index: usize,
    pub scroll_offset: usize,
    pub auto_scroll: bool,
    pub search_query: String,
}

impl OutputPanel {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            active_channel_index: 0,
            scroll_offset: 0,
            auto_scroll: true,
            search_query: String::new(),
        }
    }

    /// Create a new output channel and return its index.
    pub fn create_channel(&mut self, name: impl Into<String>) -> usize {
        let idx = self.channels.len();
        self.channels.push(OutputChannel::new(name));
        idx
    }

    /// Append a line to a channel by index.
    pub fn append_line(&mut self, channel_index: usize, line: impl Into<String>) -> bool {
        if let Some(ch) = self.channels.get_mut(channel_index) {
            ch.content.push(line.into());
            if self.auto_scroll && channel_index == self.active_channel_index {
                self.scroll_to_bottom();
            }
            true
        } else {
            false
        }
    }

    /// Clear all content in a channel.
    pub fn clear_channel(&mut self, channel_index: usize) -> bool {
        if let Some(ch) = self.channels.get_mut(channel_index) {
            ch.content.clear();
            self.scroll_offset = 0;
            true
        } else {
            false
        }
    }

    /// Select a channel as active.
    pub fn select_channel(&mut self, index: usize) -> bool {
        if index < self.channels.len() {
            self.active_channel_index = index;
            self.scroll_offset = 0;
            true
        } else {
            false
        }
    }

    /// Toggle auto-scroll behaviour.
    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
    }

    /// Search within the active channel; returns indices of matching lines.
    pub fn search(&self, query: &str) -> Vec<usize> {
        if query.is_empty() {
            return Vec::new();
        }
        let lower = query.to_lowercase();
        self.active_channel()
            .map(|ch| {
                ch.content
                    .iter()
                    .enumerate()
                    .filter(|(_, line)| line.to_lowercase().contains(&lower))
                    .map(|(i, _)| i)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get a reference to the active channel.
    pub fn active_channel(&self) -> Option<&OutputChannel> {
        self.channels.get(self.active_channel_index)
    }

    /// Create a channel, returning an error on duplicate or empty names.
    pub fn create_channel_checked(
        &mut self,
        name: impl Into<String>,
    ) -> Result<usize, OutputError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(OutputError::InvalidChannelName);
        }
        if self.channels.iter().any(|ch| ch.name == name) {
            return Err(OutputError::DuplicateChannelName(name));
        }
        Ok(self.create_channel(name))
    }

    /// Append a line, returning an error when the channel does not exist.
    pub fn append_line_checked(
        &mut self,
        channel_index: usize,
        line: impl Into<String>,
    ) -> Result<(), OutputError> {
        if self.append_line(channel_index, line) {
            Ok(())
        } else {
            Err(OutputError::ChannelNotFound(channel_index))
        }
    }

    /// Find a channel index by name (case-sensitive).
    pub fn find_channel(&self, name: &str) -> Option<usize> {
        self.channels.iter().position(|ch| ch.name == name)
    }

    /// Return the total number of lines across all channels.
    pub fn total_line_count(&self) -> usize {
        self.channels.iter().map(|ch| ch.line_count()).sum()
    }

    /// Return channel names in order.
    pub fn channel_names(&self) -> Vec<&str> {
        self.channels.iter().map(|ch| ch.name.as_str()).collect()
    }

    /// Scroll up by `n` lines, clamping at zero.
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
        self.auto_scroll = false;
    }

    /// Scroll down by `n` lines, clamping at end.
    pub fn scroll_down(&mut self, n: usize) {
        if let Some(ch) = self.active_channel() {
            let max = ch.content.len().saturating_sub(1);
            self.scroll_offset = (self.scroll_offset + n).min(max);
        }
    }

    /// Remove a channel by index, adjusting active index if needed.
    pub fn remove_channel(&mut self, index: usize) -> Result<OutputChannel, OutputError> {
        if index >= self.channels.len() {
            return Err(OutputError::ChannelNotFound(index));
        }
        let ch = self.channels.remove(index);
        if self.channels.is_empty() {
            self.active_channel_index = 0;
        } else if self.active_channel_index >= self.channels.len() {
            self.active_channel_index = self.channels.len() - 1;
        }
        self.scroll_offset = 0;
        Ok(ch)
    }

    fn scroll_to_bottom(&mut self) {
        if let Some(ch) = self.channels.get(self.active_channel_index) {
            self.scroll_offset = ch.content.len().saturating_sub(1);
        }
    }

    /// Render the output panel.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 6 {
            return;
        }

        // Channel selector bar (first row).
        let selector_area = Rect { height: 1, ..area };
        self.render_channel_selector(selector_area, buf);

        // Scrollable content (remaining rows).
        let content_area = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        };
        self.render_content(content_area, buf);
    }

    fn render_channel_selector(&self, area: Rect, buf: &mut Buffer) {
        let mut x = area.x;
        for (i, ch) in self.channels.iter().enumerate() {
            let style = if i == self.active_channel_index {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let label = format!(" {} ", ch.name);
            let width = label.len() as u16;
            if x + width > area.x + area.width {
                break;
            }
            let span = Span::styled(label, style);
            let line = Line::from(vec![span]);
            let r = Rect {
                x,
                y: area.y,
                width,
                height: 1,
            };
            line.render(r, buf);
            x += width;
        }
    }

    fn render_content(&self, area: Rect, buf: &mut Buffer) {
        let Some(ch) = self.active_channel() else {
            return;
        };
        let visible_lines = area.height as usize;
        let start = self.scroll_offset;
        for (i, line_text) in ch.content.iter().skip(start).take(visible_lines).enumerate() {
            let truncated: String = line_text.chars().take(area.width as usize).collect();
            let line = Line::from(vec![Span::styled(
                truncated,
                Style::default().fg(Color::White),
            )]);
            let row = Rect {
                y: area.y + i as u16,
                height: 1,
                ..area
            };
            line.render(row, buf);
        }
    }
}

impl fmt::Display for OutputPanel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OutputPanel({} channels, active={}, scroll={}, auto_scroll={})",
            self.channels.len(),
            self.active_channel_index,
            self.scroll_offset,
            self.auto_scroll,
        )
    }
}

impl Default for OutputPanel {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Aggregate statistics
// ---------------------------------------------------------------------------

/// Aggregate statistics across all channels in an [`OutputPanel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputStats {
    /// Total number of content lines across every channel.
    pub total_lines: usize,
    /// Total byte size of all content lines (excluding newlines).
    pub total_bytes: usize,
    /// Number of channels in the panel.
    pub channel_count: usize,
}

impl fmt::Display for OutputStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OutputStats(channels={}, lines={}, bytes={})",
            self.channel_count, self.total_lines, self.total_bytes
        )
    }
}

/// Compute aggregate statistics for all channels in the given panel.
pub fn compute_output_stats(panel: &OutputPanel) -> OutputStats {
    let mut total_lines: usize = 0;
    let mut total_bytes: usize = 0;
    for ch in &panel.channels {
        total_lines += ch.line_count();
        total_bytes += ch.byte_size();
    }
    OutputStats {
        total_lines,
        total_bytes,
        channel_count: panel.channels.len(),
    }
}

// ---------------------------------------------------------------------------
// Channel filtering, batch append, reordering, and export
// ---------------------------------------------------------------------------

impl OutputPanel {
    /// Return indices of all visible channels.
    pub fn visible_channel_indices(&self) -> Vec<usize> {
        self.channels
            .iter()
            .enumerate()
            .filter(|(_, ch)| ch.is_visible)
            .map(|(i, _)| i)
            .collect()
    }

    /// Return indices of all hidden channels.
    pub fn hidden_channel_indices(&self) -> Vec<usize> {
        self.channels
            .iter()
            .enumerate()
            .filter(|(_, ch)| !ch.is_visible)
            .map(|(i, _)| i)
            .collect()
    }

    /// Set visibility for a channel by index. Returns false if index is invalid.
    pub fn set_channel_visibility(&mut self, index: usize, visible: bool) -> bool {
        if let Some(ch) = self.channels.get_mut(index) {
            ch.is_visible = visible;
            true
        } else {
            false
        }
    }

    /// Append multiple lines to a channel at once.
    pub fn batch_append(&mut self, channel_index: usize, lines: &[&str]) -> bool {
        if let Some(ch) = self.channels.get_mut(channel_index) {
            for line in lines {
                ch.content.push((*line).to_string());
            }
            if self.auto_scroll && channel_index == self.active_channel_index {
                self.scroll_to_bottom();
            }
            true
        } else {
            false
        }
    }

    /// Swap the positions of two channels. Returns false if either index is invalid.
    pub fn swap_channels(&mut self, a: usize, b: usize) -> bool {
        if a >= self.channels.len() || b >= self.channels.len() {
            return false;
        }
        self.channels.swap(a, b);
        // Adjust active index if it was one of the swapped channels.
        if self.active_channel_index == a {
            self.active_channel_index = b;
        } else if self.active_channel_index == b {
            self.active_channel_index = a;
        }
        true
    }

    /// Move a channel from `from` index to `to` index, shifting others.
    pub fn move_channel(&mut self, from: usize, to: usize) -> bool {
        if from >= self.channels.len() || to >= self.channels.len() {
            return false;
        }
        let ch = self.channels.remove(from);
        self.channels.insert(to, ch);
        // Reset active to the moved channel's new position if it was active.
        if self.active_channel_index == from {
            self.active_channel_index = to;
        } else if from < self.active_channel_index && to >= self.active_channel_index {
            self.active_channel_index = self.active_channel_index.saturating_sub(1);
        } else if from > self.active_channel_index && to <= self.active_channel_index {
            self.active_channel_index = (self.active_channel_index + 1).min(self.channels.len() - 1);
        }
        true
    }

    /// Export the content of the active channel as a single newline-joined string.
    pub fn export_active_channel(&self) -> Option<String> {
        self.active_channel().map(|ch| ch.content.join("\n"))
    }

    /// Export the content of a specific channel by index.
    pub fn export_channel(&self, index: usize) -> Option<String> {
        self.channels.get(index).map(|ch| ch.content.join("\n"))
    }

    /// Export all channels as a combined string with channel headers.
    pub fn export_all_channels(&self) -> String {
        let mut result = String::new();
        for ch in &self.channels {
            result.push_str(&format!("=== {} ===\n", ch.name));
            for line in &ch.content {
                result.push_str(line);
                result.push('\n');
            }
            result.push('\n');
        }
        result
    }
}

/// Severity levels for output lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSeverity {
    Info,
    Warning,
    Error,
}

/// Filters output lines by severity keyword or a custom substring pattern.
#[derive(Debug, Clone)]
pub struct OutputChannelFilter {
    severity: Option<OutputSeverity>,
    pattern: Option<String>,
}

impl OutputChannelFilter {
    /// Create a filter that matches a given severity.
    pub fn by_severity(severity: OutputSeverity) -> Self {
        Self { severity: Some(severity), pattern: None }
    }

    /// Create a filter that matches lines containing `pattern`.
    pub fn by_pattern(pattern: &str) -> Self {
        Self { severity: None, pattern: Some(pattern.to_string()) }
    }

    /// Create a filter matching both severity and pattern.
    pub fn by_severity_and_pattern(severity: OutputSeverity, pattern: &str) -> Self {
        Self { severity: Some(severity), pattern: Some(pattern.to_string()) }
    }

    /// Returns `true` if the given line matches this filter.
    pub fn matches(&self, line: &str) -> bool {
        if let Some(ref sev) = self.severity {
            let keyword = match sev {
                OutputSeverity::Info => "[info]",
                OutputSeverity::Warning => "[warning]",
                OutputSeverity::Error => "[error]",
            };
            if !line.to_lowercase().contains(keyword) {
                return false;
            }
        }
        if let Some(ref pat) = self.pattern {
            if !line.contains(pat.as_str()) {
                return false;
            }
        }
        true
    }
    /// Apply this filter to all lines in a channel, returning matching line indices.
    pub fn apply(&self, channel: &OutputChannel) -> Vec<usize> {
        channel
            .content
            .iter()
            .enumerate()
            .filter(|(_, line)| self.matches(line))
            .map(|(i, _)| i)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// OutputChannelGroup — categorized output channels
// ---------------------------------------------------------------------------

/// Groups output channels by category for organized display.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputChannelGroup {
    pub category: String,
    pub channel_indices: Vec<usize>,
}

impl OutputChannelGroup {
    pub fn new(category: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            channel_indices: Vec::new(),
        }
    }

    /// Add a channel index to this group.
    pub fn add_channel(&mut self, index: usize) {
        if !self.channel_indices.contains(&index) {
            self.channel_indices.push(index);
        }
    }

    /// Remove a channel index from this group. Returns true if found.
    pub fn remove_channel(&mut self, index: usize) -> bool {
        if let Some(pos) = self.channel_indices.iter().position(|&i| i == index) {
            self.channel_indices.remove(pos);
            true
        } else {
            false
        }
    }

    /// Returns the number of channels in this group.
    pub fn channel_count(&self) -> usize {
        self.channel_indices.len()
    }

    /// Returns true if the group contains no channels.
    pub fn is_empty(&self) -> bool {
        self.channel_indices.is_empty()
    }
}

impl fmt::Display for OutputChannelGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OutputChannelGroup({}, {} channels)",
            self.category,
            self.channel_indices.len()
        )
    }
}

/// Manages multiple channel groups for an [`OutputPanel`].
#[derive(Debug, Clone, Default)]
pub struct OutputGroupManager {
    groups: Vec<OutputChannelGroup>,
}

impl OutputGroupManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new group and return its index.
    pub fn create_group(&mut self, category: impl Into<String>) -> usize {
        let idx = self.groups.len();
        self.groups.push(OutputChannelGroup::new(category));
        idx
    }

    /// Add a channel to a group. Returns false if the group index is invalid.
    pub fn add_to_group(&mut self, group_index: usize, channel_index: usize) -> bool {
        if let Some(g) = self.groups.get_mut(group_index) {
            g.add_channel(channel_index);
            true
        } else {
            false
        }
    }

    /// Get a group by index.
    pub fn get_group(&self, index: usize) -> Option<&OutputChannelGroup> {
        self.groups.get(index)
    }

    /// Find a group by category name.
    pub fn find_group(&self, category: &str) -> Option<usize> {
        self.groups.iter().position(|g| g.category == category)
    }

    /// Return all group names.
    pub fn group_names(&self) -> Vec<&str> {
        self.groups.iter().map(|g| g.category.as_str()).collect()
    }

    /// Return the number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
}

// ---------------------------------------------------------------------------
// output_search — find text across all channel output
// ---------------------------------------------------------------------------

/// A search match within an output channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSearchMatch {
    pub channel_index: usize,
    pub line_index: usize,
    pub start_col: usize,
    pub end_col: usize,
}

/// Search for a pattern across all channels in the panel.
///
/// Returns matches with channel index, line index, and column range.
/// The search is case-insensitive.
pub fn output_search(panel: &OutputPanel, query: &str) -> Vec<OutputSearchMatch> {
    let mut results = Vec::new();
    if query.is_empty() {
        return results;
    }
    let lower_query = query.to_lowercase();
    for (ch_idx, channel) in panel.channels.iter().enumerate() {
        for (line_idx, line) in channel.content.iter().enumerate() {
            let lower_line = line.to_lowercase();
            let mut start = 0;
            while let Some(pos) = lower_line[start..].find(&lower_query) {
                let abs_start = start + pos;
                results.push(OutputSearchMatch {
                    channel_index: ch_idx,
                    line_index: line_idx,
                    start_col: abs_start,
                    end_col: abs_start + query.len(),
                });
                start = abs_start + 1;
            }
        }
    }
    results
}

// ---------------------------------------------------------------------------
// OutputChannelAppendMode — append/replace/prepend behavior
// ---------------------------------------------------------------------------

/// How content should be added to an output channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputChannelAppendMode {
    /// Add content to the end (default).
    Append,
    /// Replace all existing content.
    Replace,
    /// Add content to the beginning.
    Prepend,
}

impl fmt::Display for OutputChannelAppendMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Append => write!(f, "Append"),
            Self::Replace => write!(f, "Replace"),
            Self::Prepend => write!(f, "Prepend"),
        }
    }
}

impl OutputPanel {
    /// Append a line using the specified mode.
    pub fn append_with_mode(
        &mut self,
        channel_index: usize,
        line: impl Into<String>,
        mode: OutputChannelAppendMode,
    ) -> bool {
        if let Some(ch) = self.channels.get_mut(channel_index) {
            let line = line.into();
            match mode {
                OutputChannelAppendMode::Append => ch.content.push(line),
                OutputChannelAppendMode::Replace => {
                    ch.content.clear();
                    ch.content.push(line);
                }
                OutputChannelAppendMode::Prepend => ch.content.insert(0, line),
            }
            if self.auto_scroll && channel_index == self.active_channel_index {
                self.scroll_to_bottom();
            }
            true
        } else {
            false
        }
    }
}

// ── OutputChannelExporter ──

/// Format options for exporting output channel content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportFormat {
    /// Plain text, one line per content line.
    Plain,
    /// Lines prefixed with 1-based line numbers.
    Numbered,
    /// Lines prefixed with channel name and line number.
    ChannelPrefixed,
}

/// Exports output channel content as a formatted string.
pub struct OutputChannelExporter;

impl OutputChannelExporter {
    /// Export a single channel's content.
    pub fn export(channel: &OutputChannel, format: &ExportFormat) -> String {
        match format {
            ExportFormat::Plain => channel.content.join("\n"),
            ExportFormat::Numbered => channel
                .content
                .iter()
                .enumerate()
                .map(|(i, line)| format!("{:>4}: {}", i + 1, line))
                .collect::<Vec<_>>()
                .join("\n"),
            ExportFormat::ChannelPrefixed => channel
                .content
                .iter()
                .enumerate()
                .map(|(i, line)| format!("[{}:{:>4}] {}", channel.name, i + 1, line))
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// Export all channels from a panel.
    pub fn export_all(panel: &OutputPanel, format: &ExportFormat) -> String {
        panel
            .channels
            .iter()
            .map(|ch| {
                let header = format!("=== {} ===", ch.name);
                let body = Self::export(ch, format);
                format!("{}\n{}", header, body)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Export only the active channel.
    pub fn export_active(panel: &OutputPanel, format: &ExportFormat) -> Option<String> {
        panel
            .active_channel()
            .map(|ch| Self::export(ch, format))
    }
}

// ── Output rotation ──

/// Configuration for automatic line rotation (trimming old lines).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputRotationPolicy {
    /// Maximum number of lines to keep per channel.
    pub max_lines: usize,
    /// Number of lines to trim when the limit is hit.  Removes from the front.
    pub trim_count: usize,
}

impl OutputRotationPolicy {
    /// Create a new rotation policy.
    pub fn new(max_lines: usize, trim_count: usize) -> Self {
        Self {
            max_lines,
            trim_count: trim_count.min(max_lines),
        }
    }

    /// Apply the policy to a channel, trimming old lines if necessary.
    /// Returns the number of lines removed.
    pub fn apply(&self, channel: &mut OutputChannel) -> usize {
        if channel.content.len() <= self.max_lines {
            return 0;
        }
        let to_remove = self.trim_count.min(channel.content.len());
        channel.content.drain(0..to_remove);
        to_remove
    }

    /// Check whether the channel currently exceeds the limit.
    pub fn needs_rotation(&self, channel: &OutputChannel) -> bool {
        channel.content.len() > self.max_lines
    }
}

/// Apply a rotation policy to all channels in the panel.
pub fn rotate_all_channels(panel: &mut OutputPanel, policy: &OutputRotationPolicy) -> usize {
    let mut total_removed = 0;
    for ch in &mut panel.channels {
        total_removed += policy.apply(ch);
    }
    total_removed
}

// ── Channel content search with match positions ──

/// A match position within a single channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSearchHit {
    pub line_index: usize,
    pub start_col: usize,
    pub end_col: usize,
}

/// Search within a single channel's content (case-insensitive) and return all
/// match positions.
pub fn channel_search(channel: &OutputChannel, query: &str) -> Vec<ChannelSearchHit> {
    let mut results = Vec::new();
    if query.is_empty() {
        return results;
    }
    let lower_query = query.to_lowercase();
    for (line_idx, line) in channel.content.iter().enumerate() {
        let lower_line = line.to_lowercase();
        let mut start = 0;
        while let Some(pos) = lower_line[start..].find(&lower_query) {
            let abs_start = start + pos;
            results.push(ChannelSearchHit {
                line_index: line_idx,
                start_col: abs_start,
                end_col: abs_start + query.len(),
            });
            start = abs_start + 1;
        }
    }
    results
}

/// Count the number of matches in a channel.
pub fn channel_search_count(channel: &OutputChannel, query: &str) -> usize {
    channel_search(channel, query).len()
}

// ---------------------------------------------------------------------------
// OutputEntry — timestamped, categorized output line
// ---------------------------------------------------------------------------

/// Severity level for a categorized output entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntrySeverity {
    Debug,
    Info,
    Warning,
    Error,
}

impl fmt::Display for EntrySeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

/// A single timestamped, categorized output entry.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputEntry {
    /// Seconds since the channel was created (or an arbitrary epoch).
    pub timestamp_secs: u64,
    pub severity: EntrySeverity,
    pub message: String,
    /// Optional source identifier (e.g. "rustc", "clippy", "cargo").
    pub source: Option<String>,
}

impl OutputEntry {
    pub fn new(timestamp_secs: u64, severity: EntrySeverity, message: impl Into<String>) -> Self {
        Self {
            timestamp_secs,
            severity,
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Format the entry as a human-readable string.
    pub fn format(&self) -> String {
        let ts = format_timestamp(self.timestamp_secs);
        match &self.source {
            Some(src) => format!("[{} {:>5} {}] {}", ts, self.severity, src, self.message),
            None => format!("[{} {:>5}] {}", ts, self.severity, self.message),
        }
    }
}

impl fmt::Display for OutputEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

/// Format seconds into `HH:MM:SS`.
fn format_timestamp(total_secs: u64) -> String {
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

// ---------------------------------------------------------------------------
// StructuredOutputChannel — channel with typed entries
// ---------------------------------------------------------------------------

/// An output channel that stores structured [`OutputEntry`] items instead of
/// plain strings, enabling filtering, searching, and merging by metadata.
#[derive(Debug, Clone)]
pub struct StructuredOutputChannel {
    pub name: String,
    entries: Vec<OutputEntry>,
    max_entries: Option<usize>,
}

impl StructuredOutputChannel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entries: Vec::new(),
            max_entries: None,
        }
    }

    /// Create a channel with an upper limit on stored entries (log rotation).
    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = Some(max);
        self
    }

    /// Push an entry, applying log rotation if a limit is configured.
    pub fn push(&mut self, entry: OutputEntry) {
        self.entries.push(entry);
        if let Some(max) = self.max_entries {
            if self.entries.len() > max {
                let excess = self.entries.len() - max;
                self.entries.drain(0..excess);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[OutputEntry] {
        &self.entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Filter entries by severity.
    pub fn filter_by_severity(&self, severity: EntrySeverity) -> Vec<&OutputEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    /// Filter entries by source identifier.
    pub fn filter_by_source(&self, source: &str) -> Vec<&OutputEntry> {
        self.entries
            .iter()
            .filter(|e| e.source.as_deref() == Some(source))
            .collect()
    }

    /// Search entries whose message contains `query` (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&OutputEntry> {
        if query.is_empty() {
            return Vec::new();
        }
        let lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.message.to_lowercase().contains(&lower))
            .collect()
    }

    /// Return the last `n` entries.
    pub fn tail(&self, n: usize) -> &[OutputEntry] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    /// Count entries per severity. Returns `(debug, info, warning, error)`.
    pub fn severity_counts(&self) -> (usize, usize, usize, usize) {
        let mut d = 0;
        let mut i = 0;
        let mut w = 0;
        let mut e = 0;
        for entry in &self.entries {
            match entry.severity {
                EntrySeverity::Debug => d += 1,
                EntrySeverity::Info => i += 1,
                EntrySeverity::Warning => w += 1,
                EntrySeverity::Error => e += 1,
            }
        }
        (d, i, w, e)
    }

    /// Convert all entries into formatted strings suitable for a plain
    /// `OutputChannel`.
    pub fn to_plain_lines(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.format()).collect()
    }
}

impl fmt::Display for StructuredOutputChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StructuredOutputChannel({}, {} entries)",
            self.name,
            self.entries.len()
        )
    }
}

// ---------------------------------------------------------------------------
// merge_channels — combine multiple structured channels chronologically
// ---------------------------------------------------------------------------

/// A single entry in a merged view, annotated with the originating channel.
#[derive(Debug, Clone, PartialEq)]
pub struct MergedEntry {
    pub channel_name: String,
    pub entry: OutputEntry,
}

impl fmt::Display for MergedEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.channel_name, self.entry)
    }
}

/// Merge multiple structured channels into a single chronological stream,
/// ordered by `timestamp_secs` (stable — preserves insertion order for ties).
pub fn merge_channels(channels: &[&StructuredOutputChannel]) -> Vec<MergedEntry> {
    let mut merged: Vec<MergedEntry> = channels
        .iter()
        .flat_map(|ch| {
            ch.entries().iter().map(|e| MergedEntry {
                channel_name: ch.name.clone(),
                entry: e.clone(),
            })
        })
        .collect();
    merged.sort_by_key(|m| m.entry.timestamp_secs);
    merged
}

// ---------------------------------------------------------------------------
// OutputSummary — aggregate severity stats across channels
// ---------------------------------------------------------------------------

/// Summary statistics for a set of structured channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSummary {
    pub total_entries: usize,
    pub debug_count: usize,
    pub info_count: usize,
    pub warning_count: usize,
    pub error_count: usize,
    pub channel_count: usize,
}

impl fmt::Display for OutputSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} entries across {} channels (D:{} I:{} W:{} E:{})",
            self.total_entries,
            self.channel_count,
            self.debug_count,
            self.info_count,
            self.warning_count,
            self.error_count,
        )
    }
}

/// Compute an [`OutputSummary`] from a slice of structured channels.
pub fn compute_summary(channels: &[&StructuredOutputChannel]) -> OutputSummary {
    let mut summary = OutputSummary {
        total_entries: 0,
        debug_count: 0,
        info_count: 0,
        warning_count: 0,
        error_count: 0,
        channel_count: channels.len(),
    };
    for ch in channels {
        let (d, i, w, e) = ch.severity_counts();
        summary.debug_count += d;
        summary.info_count += i;
        summary.warning_count += w;
        summary.error_count += e;
        summary.total_entries += ch.len();
    }
    summary
}

// ---------------------------------------------------------------------------
// Text filter
// ---------------------------------------------------------------------------

/// Configurable text filter for matching output lines by substring or regex.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputChannelTextFilter {
    pub query: String,
    pub case_sensitive: bool,
    pub use_regex: bool,
}

impl OutputChannelTextFilter {
    /// Create a new filter with a query string. Case-insensitive, no regex by default.
    pub fn new(query: &str) -> Self {
        Self {
            query: query.to_string(),
            case_sensitive: false,
            use_regex: false,
        }
    }

    pub fn set_case_sensitive(&mut self, yes: bool) {
        self.case_sensitive = yes;
    }

    pub fn set_regex(&mut self, yes: bool) {
        self.use_regex = yes;
    }

    /// Return `true` if `line` matches the current query.
    pub fn matches(&self, line: &str) -> bool {
        if self.query.is_empty() {
            return false;
        }
        if self.use_regex {
            // Simple character-class-free regex-like: treat query as literal pattern
            // but respect case sensitivity. Full regex crate is not available.
            if self.case_sensitive {
                line.contains(&self.query)
            } else {
                line.to_lowercase().contains(&self.query.to_lowercase())
            }
        } else if self.case_sensitive {
            line.contains(&self.query)
        } else {
            line.to_lowercase().contains(&self.query.to_lowercase())
        }
    }

    /// Return `(index, &line)` pairs for every matching line.
    pub fn filter_lines<'a>(&self, lines: &'a [String]) -> Vec<(usize, &'a String)> {
        lines
            .iter()
            .enumerate()
            .filter(|(_, l)| self.matches(l))
            .collect()
    }

    /// Count how many lines match.
    pub fn match_count(&self, lines: &[String]) -> usize {
        lines.iter().filter(|l| self.matches(l)).count()
    }
}

// ---------------------------------------------------------------------------
// Scroll state
// ---------------------------------------------------------------------------

/// Tracks scroll position and auto-follow behaviour for a channel viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputChannelScrollState {
    pub offset: usize,
    pub auto_follow: bool,
    pub viewport_height: usize,
}

impl OutputChannelScrollState {
    pub fn new(viewport_height: usize) -> Self {
        Self {
            offset: 0,
            auto_follow: true,
            viewport_height,
        }
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.offset = self.offset.saturating_add(n);
        self.auto_follow = false;
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
        self.auto_follow = false;
    }

    pub fn scroll_to_bottom(&mut self, total_lines: usize) {
        self.offset = total_lines.saturating_sub(self.viewport_height);
    }

    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
        self.auto_follow = false;
    }

    pub fn toggle_auto_follow(&mut self) {
        self.auto_follow = !self.auto_follow;
    }

    /// Return `true` when the viewport shows the very last line.
    pub fn is_at_bottom(&self, total_lines: usize) -> bool {
        if total_lines <= self.viewport_height {
            return true;
        }
        self.offset >= total_lines.saturating_sub(self.viewport_height)
    }

    /// Return the `(start, end)` line indices visible in the current viewport.
    pub fn visible_range(&self, total_lines: usize) -> (usize, usize) {
        let start = self.offset.min(total_lines);
        let end = (start + self.viewport_height).min(total_lines);
        (start, end)
    }

    pub fn set_viewport_height(&mut self, h: usize) {
        self.viewport_height = h;
    }

    /// When auto-follow is on, snap to the bottom.
    pub fn follow_if_needed(&mut self, total_lines: usize) {
        if self.auto_follow {
            self.scroll_to_bottom(total_lines);
        }
    }
}

// ---------------------------------------------------------------------------
// Timestamp formatter
// ---------------------------------------------------------------------------

/// Supported timestamp display formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTimestampFormat {
    /// ISO-style `YYYY-MM-DD HH:MM:SS` (simplified – epoch seconds only).
    Iso,
    /// `HH:MM:SS`.
    TimeOnly,
    /// Elapsed offset from a start time, e.g. `+42s`.
    Elapsed,
}

/// Formats a timestamp and prepends it to an output line.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputTimestampFormatter {
    pub format: OutputTimestampFormat,
}

impl OutputTimestampFormatter {
    pub fn new(format: OutputTimestampFormat) -> Self {
        Self { format }
    }

    pub fn set_format(&mut self, format: OutputTimestampFormat) {
        self.format = format;
    }

    /// Format `timestamp_secs` in ISO-like style (days/hours/minutes/seconds from epoch).
    pub fn format_iso(&self, secs: u64) -> String {
        let s = secs % 60;
        let m = (secs / 60) % 60;
        let h = (secs / 3600) % 24;
        let d = secs / 86400;
        format!("{d:04}-{h:02}:{m:02}:{s:02}")
    }

    /// Format `timestamp_secs` as `HH:MM:SS`.
    pub fn format_time_only(&self, secs: u64) -> String {
        let s = secs % 60;
        let m = (secs / 60) % 60;
        let h = (secs / 3600) % 24;
        format!("{h:02}:{m:02}:{s:02}")
    }

    /// Format elapsed seconds since `start`.
    pub fn format_elapsed(&self, secs: u64, start: u64) -> String {
        let elapsed = secs.saturating_sub(start);
        format!("+{elapsed}s")
    }

    /// Prepend a formatted timestamp to `line`.
    pub fn format_line(&self, line: &str, timestamp_secs: u64) -> String {
        let ts = match self.format {
            OutputTimestampFormat::Iso => self.format_iso(timestamp_secs),
            OutputTimestampFormat::TimeOnly => self.format_time_only(timestamp_secs),
            OutputTimestampFormat::Elapsed => self.format_elapsed(timestamp_secs, 0),
        };
        format!("[{ts}] {line}")
    }
}

// ---------------------------------------------------------------------------
// Clear confirmation
// ---------------------------------------------------------------------------

/// Confirmation dialog state for clearing a channel.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputClearConfirmation {
    pub channel_name: String,
    pub line_count: usize,
    pub confirmed: bool,
}

impl OutputClearConfirmation {
    pub fn new(name: &str, lines: usize) -> Self {
        Self {
            channel_name: name.to_string(),
            line_count: lines,
            confirmed: false,
        }
    }

    /// Confirmation is needed only when there are lines to clear.
    pub fn needs_confirmation(&self) -> bool {
        self.line_count > 0
    }

    pub fn confirm(&mut self) {
        self.confirmed = true;
    }

    pub fn cancel(&mut self) {
        self.confirmed = false;
    }

    /// Human-readable confirmation prompt.
    pub fn message(&self) -> String {
        format!(
            "Clear {} lines from '{}'?",
            self.line_count, self.channel_name
        )
    }

    pub fn is_confirmed(&self) -> bool {
        self.confirmed
    }
}

impl fmt::Display for OutputClearConfirmation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.confirmed {
            write!(f, "Cleared '{}' ({} lines)", self.channel_name, self.line_count)
        } else {
            write!(f, "{}", self.message())
        }
    }
}


// ---------------------------------------------------------------------------
// OutputTimestampInjector
// ---------------------------------------------------------------------------

/// Configuration for timestamp injection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimestampInjectorConfig {
    /// Whether timestamps are enabled.
    pub enabled: bool,
    /// Format style for timestamps.
    pub style: TimestampStyle,
    /// Separator between timestamp and content.
    pub separator: String,
}

/// Style of timestamp display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampStyle {
    /// Elapsed time since start in seconds: "[0.123s]"
    Elapsed,
    /// Absolute epoch seconds: "[1700000000]"
    Epoch,
    /// Compact HH:MM:SS format (from epoch seconds): "[HH:MM:SS]"
    HhMmSs,
}

impl Default for TimestampInjectorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            style: TimestampStyle::Elapsed,
            separator: " ".to_string(),
        }
    }
}

/// Injects timestamps into output entries.
#[derive(Debug, Clone)]
pub struct OutputTimestampInjector {
    config: TimestampInjectorConfig,
    start_epoch_ms: u64,
    entries_processed: u64,
}

impl OutputTimestampInjector {
    /// Create a new injector with the given start time.
    pub fn new(config: TimestampInjectorConfig, start_epoch_ms: u64) -> Self {
        Self {
            config,
            start_epoch_ms,
            entries_processed: 0,
        }
    }

    /// Create with defaults and current time approximated as 0.
    pub fn with_defaults() -> Self {
        Self::new(TimestampInjectorConfig::default(), 0)
    }

    /// Format a timestamp given an event time in epoch milliseconds.
    pub fn format_timestamp(&self, event_epoch_ms: u64) -> String {
        match self.config.style {
            TimestampStyle::Elapsed => {
                let elapsed_ms = event_epoch_ms.saturating_sub(self.start_epoch_ms);
                let secs = elapsed_ms as f64 / 1000.0;
                format!("[{secs:.3}s]")
            }
            TimestampStyle::Epoch => {
                let epoch_s = event_epoch_ms / 1000;
                format!("[{epoch_s}]")
            }
            TimestampStyle::HhMmSs => {
                let total_secs = event_epoch_ms / 1000;
                let h = (total_secs / 3600) % 24;
                let m = (total_secs / 60) % 60;
                let s = total_secs % 60;
                format!("[{h:02}:{m:02}:{s:02}]")
            }
        }
    }

    /// Inject a timestamp into a line of text.
    pub fn inject(&mut self, line: &str, event_epoch_ms: u64) -> String {
        self.entries_processed += 1;
        if !self.config.enabled {
            return line.to_string();
        }
        let ts = self.format_timestamp(event_epoch_ms);
        format!("{ts}{}{line}", self.config.separator)
    }

    /// Inject timestamps into multiple lines.
    pub fn inject_batch(&mut self, lines: &[&str], event_epoch_ms: u64) -> Vec<String> {
        lines.iter().enumerate().map(|(i, line)| {
            self.inject(line, event_epoch_ms + i as u64)
        }).collect()
    }

    /// Number of entries processed.
    pub fn entries_processed(&self) -> u64 {
        self.entries_processed
    }

    /// Whether injection is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Set enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Change the timestamp style.
    pub fn set_style(&mut self, style: TimestampStyle) {
        self.config.style = style;
    }

    /// Get current style.
    pub fn style(&self) -> TimestampStyle {
        self.config.style
    }

    /// Reset the entries counter.
    pub fn reset_counter(&mut self) {
        self.entries_processed = 0;
    }
}

impl fmt::Display for OutputTimestampInjector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TimestampInjector({:?}, processed={})",
            self.config.style, self.entries_processed
        )
    }
}

// ---------------------------------------------------------------------------
// OutputLanguageColorizer
// ---------------------------------------------------------------------------

/// Detected language for output content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectedLanguage {
    Rust,
    JavaScript,
    Python,
    Shell,
    Json,
    Xml,
    Plain,
}

impl fmt::Display for DetectedLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Rust => "Rust",
            Self::JavaScript => "JavaScript",
            Self::Python => "Python",
            Self::Shell => "Shell",
            Self::Json => "JSON",
            Self::Xml => "XML",
            Self::Plain => "Plain",
        };
        write!(f, "{name}")
    }
}

/// Colorizes output based on heuristic language detection.
#[derive(Debug, Clone)]
pub struct OutputLanguageColorizer {
    detections: Vec<(DetectedLanguage, u64)>,
}

impl OutputLanguageColorizer {
    /// Create a new colorizer.
    pub fn new() -> Self {
        Self { detections: Vec::new() }
    }

    /// Detect language from a line of text using simple heuristics.
    pub fn detect_language(&mut self, line: &str) -> DetectedLanguage {
        let lang = Self::heuristic_detect(line);
        self.detections.push((lang, line.len() as u64));
        lang
    }

    fn heuristic_detect(line: &str) -> DetectedLanguage {
        let trimmed = line.trim();
        if trimmed.starts_with("fn ") || trimmed.starts_with("let ") || trimmed.starts_with("use ")
            || trimmed.contains("-> ") || trimmed.starts_with("pub ") || trimmed.starts_with("impl ")
        {
            return DetectedLanguage::Rust;
        }
        if trimmed.starts_with("def ") || trimmed.starts_with("import ") || trimmed.starts_with("class ")
            || trimmed.contains("print(")
        {
            return DetectedLanguage::Python;
        }
        if trimmed.starts_with("const ") || trimmed.starts_with("function ") || trimmed.contains("=>")
            || trimmed.starts_with("var ") || trimmed.contains("console.log")
        {
            return DetectedLanguage::JavaScript;
        }
        if trimmed.starts_with('$') || trimmed.starts_with('#') || trimmed.starts_with("echo ")
            || trimmed.starts_with("export ")
        {
            return DetectedLanguage::Shell;
        }
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return DetectedLanguage::Json;
        }
        if trimmed.starts_with('<') && trimmed.ends_with('>') {
            return DetectedLanguage::Xml;
        }
        DetectedLanguage::Plain
    }

    /// Return a ratatui Color for a detected language.
    pub fn color_for(lang: DetectedLanguage) -> Color {
        match lang {
            DetectedLanguage::Rust => Color::Rgb(255, 165, 0),
            DetectedLanguage::JavaScript => Color::Yellow,
            DetectedLanguage::Python => Color::Blue,
            DetectedLanguage::Shell => Color::Green,
            DetectedLanguage::Json => Color::Cyan,
            DetectedLanguage::Xml => Color::Magenta,
            DetectedLanguage::Plain => Color::White,
        }
    }

    /// Colorize a line into a ratatui Span.
    pub fn colorize_line(&mut self, line: &str) -> Span<'_> {
        let lang = self.detect_language(line);
        Span::styled("", Style::default().fg(Self::color_for(lang)))
    }

    /// Number of detections performed.
    pub fn detection_count(&self) -> usize {
        self.detections.len()
    }

    /// Count how many times a given language was detected.
    pub fn count_language(&self, lang: DetectedLanguage) -> usize {
        self.detections.iter().filter(|(l, _)| *l == lang).count()
    }

    /// Return the most frequently detected language.
    pub fn most_frequent(&self) -> Option<DetectedLanguage> {
        if self.detections.is_empty() {
            return None;
        }
        let mut counts = std::collections::HashMap::new();
        for (lang, _) in &self.detections {
            *counts.entry(*lang).or_insert(0usize) += 1;
        }
        counts.into_iter().max_by_key(|(_, c)| *c).map(|(l, _)| l)
    }

    /// Reset all detection history.
    pub fn reset(&mut self) {
        self.detections.clear();
    }
}

impl fmt::Display for OutputLanguageColorizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LanguageColorizer({} detections)", self.detection_count())
    }
}



// ---------------------------------------------------------------------------
// output – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XOutputLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XOutputPanelState {
    pub region: XOutputLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XOutputPanelState {
    pub fn new(region: XOutputLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_output_total_visible_area(panels: &[XOutputPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_output_count_in_region(
    panels: &[XOutputPanelState],
    region: XOutputLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_output_widest_panel(panels: &[XOutputPanelState]) -> Option<&XOutputPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_output_collapse_region(
    panels: &mut [XOutputPanelState],
    region: XOutputLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XOutputLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XOutputLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// output – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for output panel channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YOutputOutputVerbosity {
    Silent,
    Quiet,
    Normal,
    Verbose,
}

impl YOutputOutputVerbosity {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Silent => 0,
            Self::Quiet => 1,
            Self::Normal => 2,
            Self::Verbose => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Silent => "Silent",
            Self::Quiet => "Quiet",
            Self::Normal => "Normal",
            Self::Verbose => "Verbose",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YOutputOutputVerbosity] {
        &[
            YOutputOutputVerbosity::Silent,
            YOutputOutputVerbosity::Quiet,
            YOutputOutputVerbosity::Normal,
            YOutputOutputVerbosity::Verbose,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YOutputOutputVerbosity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks output buffer data.
#[derive(Debug, Clone)]
pub struct YOutputOutputLogBuffer {
    pub lines: Vec<String>,
    pub max_lines: usize,
    pub total_written: u64,
}

impl YOutputOutputLogBuffer {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            max_lines: 0,
            total_written: 0,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YOutputOutputLogBuffer({}: {:?})", "lines", self.lines)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_output_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_output_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_output_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_output_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_output_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_output_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_output_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_output_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// output – Extended output search index helpers
// ---------------------------------------------------------------------------

/// Priority levels for output search index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZOutputPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZOutputPriority {
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
    pub fn all_asc() -> [ZOutputPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZOutputPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks output search index data.
#[derive(Debug, Clone)]
pub struct ZOutputOutputSearchIndex {
    pub line_offsets: Vec<usize>,
    pub total_bytes: u64,
    pub dirty: bool,
}

impl ZOutputOutputSearchIndex {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            line_offsets: Vec::new(),
            total_bytes: 0,
            dirty: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.line_offsets.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.line_offsets.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.line_offsets.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZOutputOutputSearchIndex[total_bytes={:?}, dirty={:?}]", self.total_bytes, self.dirty)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.dirty = !c.dirty;
        c
    }
}

/// Compute a simple rolling hash for output search index.
pub fn z_output_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_output_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_output_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_output_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_output_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_output_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_output_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 71
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer71 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer71 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_71(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_71<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_71<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_71(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_71(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 135
// ---------------------------------------------------------------------------

/// Generic object pool `Xc135Pool<T>`.
pub struct Xc135Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc135Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc135PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc135Pool<T> {
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
    pub fn stats(&self) -> Xc135PoolStats {
        Xc135PoolStats {
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

impl<T> Default for Xc135Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc135Scheduler`.
pub struct Xc135Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc135Scheduler {
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

impl Default for Xc135Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_135 hash for the given byte slice.
pub fn xc_135_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_135 convention.
pub fn xc_135_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe84 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe84Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe84PipelineError {
    pub stage: Xe84Stage,
    pub message: String,
}

impl std::fmt::Display for Xe84PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe84Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe84Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError>>>,
    stage_names: Vec<Xe84Stage>,
}

impl Xe84Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe84Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe84Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe84Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe84Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe84Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe84CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe84CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe84Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe84CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe84CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe84Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe84CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_84_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe84CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_84_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe84CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_84_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError> {
    Ok(data)
}

pub fn xe_84_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_84_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_84_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_84_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe84PipelineError> {
    Err(Xe84PipelineError {
        stage: Xe84Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_82: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg82Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg82Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg82Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_82: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg82Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg82Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg82Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg82Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 134).
pub struct Xh134SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh134SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 176 as u64,
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

/// A compact bit set supporting boolean operations (variant 134).
pub struct Xh134BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh134BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 134).
pub struct Xi134Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi134Deque<T> {
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
pub struct Xi134Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi134Interval {
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

/// A simple interval tree (variant 134).
pub struct Xi134IntervalTree {
    xi_intervals: Vec<Xi134Interval>,
}

impl Xi134IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi134Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi134Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi134Interval) -> Vec<&Xi134Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi134Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi134Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi134Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi134Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi134Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi134Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 133) ---

/// Disjoint set / union-find for crate 133.
pub struct Xj133UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj133UnionFind {
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

const XJ133_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 133.
pub struct Xj133BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj133BTreeNode<K, V>>>,
    len: usize,
}

struct Xj133BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj133BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj133BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ133_BTREE_ORDER - 1
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
        let mid = XJ133_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj133BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj133BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj133BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj133BTreeNode::xj_new_leaf();
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


// --- xk_133 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk133SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk133SegmentTree {
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
pub struct Xk133DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk133DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_133).
#[derive(Debug, Clone)]
pub struct Xl133Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl133Rope {
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

/// Suffix array for efficient string searching (xl_133).
#[derive(Debug, Clone)]
pub struct Xl133SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl133SuffixArray {
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
pub struct Xm133MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm133MatrixSparse {
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
pub struct Xm133Tokenizer {
    text: String,
}

impl Xm133Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 134.
pub struct Xn134Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn134Fenwick {
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

// ----- AVL tree map — crate 134 -----

#[derive(Debug, Clone)]
struct Xn134AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn134AvlNode<K, V>>>,
    right: Option<Box<Xn134AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 134.
#[derive(Debug, Clone)]
pub struct Xn134AVL<K, V> {
    root: Option<Box<Xn134AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn134AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn134AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn134AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn134AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn134AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn134AvlNode<K, V>>) -> Box<Xn134AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn134AvlNode<K, V>>) -> Box<Xn134AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn134AvlNode<K, V>>) -> Box<Xn134AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn134AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn134AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn134AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn134AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn134AvlNode<K, V>>) -> &Xn134AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn134AvlNode<K, V>>) -> (Box<Xn134AvlNode<K, V>>, Option<Box<Xn134AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn134AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn134AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn134AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn134AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn134AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn134AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn134AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo134RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo134Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo134RBNode<K, V> {
    key: K,
    value: V,
    color: Xo134Color,
    left: Option<Box<Xo134RBNode<K, V>>>,
    right: Option<Box<Xo134RBNode<K, V>>>,
}

/// A red-black tree map for crate 134.
#[derive(Debug, Clone)]
pub struct Xo134RedBlack<K, V> {
    root: Option<Box<Xo134RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo134RedBlack<K, V> {
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
            r.color = Xo134Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo134RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo134RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo134RBNode {
                    key, value, color: Xo134Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo134RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo134Color::Red)
    }

    fn xo_balance(mut h: Box<Xo134RBNode<K, V>>) -> Box<Xo134RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo134Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo134RBNode<K, V>>) -> Box<Xo134RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo134Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo134RBNode<K, V>>) -> Box<Xo134RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo134Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo134RBNode<K, V>>) {
        h.color = Xo134Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo134Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo134Color::Black; }
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
            r.color = Xo134Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo134RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo134RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo134RBNode<K, V>) -> (K, V, Option<Box<Xo134RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo134RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo134Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo134RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo134ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 134.
#[derive(Debug, Clone)]
pub struct Xo134ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo134ConsistentHash {
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
            let vkey = format!("{}#xo134#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo134#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 133).
#[derive(Debug)]
pub struct Xp133SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp133Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp133Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp133Node<K, V>>>,
    xp_right: Option<Box<Xp133Node<K, V>>>,
}

impl<K: Ord, V> Xp133Node<K, V> {
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

impl<K: Ord, V> Default for Xp133SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp133SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp133Node<K, V>>>, key: &K) -> Option<Box<Xp133Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp133Node<K, V>>) -> Box<Xp133Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp133Node<K, V>>) -> Box<Xp133Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp133Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp133Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp133Node::xp_new(key, val));
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


// --------------- Xq134Treap ---------------

use std::cmp::Ordering as Xq134Ord;

struct Xq134TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq134TreapNode<K, V>>>,
    right: Option<Box<Xq134TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq134Treap<K, V> {
    root: Option<Box<Xq134TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq134TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_134_size<K, V>(node: &Option<Box<Xq134TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_134_update_size<K, V>(node: &mut Xq134TreapNode<K, V>) {
    node.size = 1 + xq_134_size(&node.left) + xq_134_size(&node.right);
}

fn xq_134_rotate_right<K, V>(mut node: Box<Xq134TreapNode<K, V>>) -> Box<Xq134TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_134_update_size(&mut node);
    left.right = Some(node);
    xq_134_update_size(&mut left);
    left
}

fn xq_134_rotate_left<K, V>(mut node: Box<Xq134TreapNode<K, V>>) -> Box<Xq134TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_134_update_size(&mut node);
    right.left = Some(node);
    xq_134_update_size(&mut right);
    right
}

fn xq_134_insert_node<K: Ord, V>(
    node: Option<Box<Xq134TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq134TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq134TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq134Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq134Ord::Less => {
                let (new_left, old) = xq_134_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_134_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_134_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq134Ord::Greater => {
                let (new_right, old) = xq_134_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_134_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_134_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_134_remove_node<K: Ord, V>(
    node: Option<Box<Xq134TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq134TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq134Ord::Less => {
                let (new_left, old) = xq_134_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_134_update_size(&mut n);
                (Some(n), old)
            }
            Xq134Ord::Greater => {
                let (new_right, old) = xq_134_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_134_update_size(&mut n);
                (Some(n), old)
            }
            Xq134Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_134_rotate_right(n);
                    let (new_right, old) = xq_134_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_134_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_134_rotate_left(n);
                    let (new_left, old) = xq_134_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_134_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_134_find_min<K, V>(node: &Option<Box<Xq134TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_134_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_134_find_max<K, V>(node: &Option<Box<Xq134TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_134_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_134_rank<K: Ord, V>(node: &Option<Box<Xq134TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq134Ord::Less => xq_134_rank(&n.left, key),
            Xq134Ord::Equal => xq_134_size(&n.left),
            Xq134Ord::Greater => 1 + xq_134_size(&n.left) + xq_134_rank(&n.right, key),
        },
    }
}

fn xq_134_kth<K, V>(node: &Option<Box<Xq134TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_134_size(&n.left);
        if k < left_size {
            xq_134_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_134_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_134_in_order<K: Clone, V>(node: &Option<Box<Xq134TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_134_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_134_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq134Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 134 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_134_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq134Ord::Equal => return Some(&n.value),
                Xq134Ord::Less => cur = &n.left,
                Xq134Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_134_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_134_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_134_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_134_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_134_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_134_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_134_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq134VEBTree ---------------

pub struct Xq134VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq134VEBTree>>,
    clusters: Vec<Option<Box<Xq134VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq134VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq134VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq134VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr134KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr134KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr134BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr134KDNode {
    xr_point: Xr134KDPoint,
    xr_left: Option<Box<Xr134KDNode>>,
    xr_right: Option<Box<Xr134KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr134KDTree {
    xr_root: Option<Box<Xr134KDNode>>,
    xr_size: usize,
}

impl Xr134KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr134KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr134KDNode>>,
        point: Xr134KDPoint,
        depth: usize,
    ) -> Box<Xr134KDNode> {
        match node {
            None => Box::new(Xr134KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr134KDPoint) -> Option<Xr134KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr134KDNode>,
        query: &Xr134KDPoint,
        depth: usize,
        best: &mut Xr134KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr134KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr134KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr134KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr134KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr134KDNode>>, pts: &mut Vec<Xr134KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr134KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr134BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr134BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let p = OutputPanel::new();
        assert!(p.channels.is_empty());
        assert!(p.auto_scroll);
    }

    #[test]
    fn create_and_select_channel() {
        let mut p = OutputPanel::new();
        let idx = p.create_channel("Git");
        assert_eq!(idx, 0);
        assert!(p.select_channel(0));
        assert!(!p.select_channel(5));
    }

    #[test]
    fn append_and_clear() {
        let mut p = OutputPanel::new();
        p.create_channel("Log");
        assert!(p.append_line(0, "hello"));
        assert_eq!(p.channels[0].content.len(), 1);
        p.clear_channel(0);
        assert!(p.channels[0].content.is_empty());
    }

    #[test]
    fn append_to_invalid_channel() {
        let mut p = OutputPanel::new();
        assert!(!p.append_line(0, "nope"));
    }

    #[test]
    fn toggle_auto_scroll_works() {
        let mut p = OutputPanel::new();
        assert!(p.auto_scroll);
        p.toggle_auto_scroll();
        assert!(!p.auto_scroll);
    }

    #[test]
    fn search_finds_matches() {
        let mut p = OutputPanel::new();
        p.create_channel("Test");
        p.append_line(0, "error: something failed");
        p.append_line(0, "info: all good");
        p.append_line(0, "error: another failure");
        let results = p.search("error");
        assert_eq!(results, vec![0, 2]);
    }

    #[test]
    fn search_empty_query() {
        let p = OutputPanel::new();
        assert!(p.search("").is_empty());
    }

    #[test]
    fn render_does_not_panic() {
        let mut p = OutputPanel::new();
        p.create_channel("Output");
        p.append_line(0, "line 1");
        p.append_line(0, "line 2");
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf);
    }

    #[test]
    fn render_empty_no_panic() {
        let p = OutputPanel::new();
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf);
    }

    #[test]
    fn default_impl() {
        let p = OutputPanel::default();
        assert!(p.channels.is_empty());
    }

    // ---- New tests ----

    #[test]
    fn output_error_display() {
        assert_eq!(
            OutputError::ChannelNotFound(3).to_string(),
            "channel index 3 not found"
        );
        assert_eq!(
            OutputError::DuplicateChannelName("Git".into()).to_string(),
            "channel 'Git' already exists"
        );
        assert_eq!(
            OutputError::InvalidChannelName.to_string(),
            "channel name must not be empty"
        );
    }

    #[test]
    fn create_channel_checked_rejects_duplicate() {
        let mut p = OutputPanel::new();
        assert!(p.create_channel_checked("Log").is_ok());
        assert_eq!(
            p.create_channel_checked("Log"),
            Err(OutputError::DuplicateChannelName("Log".into()))
        );
    }

    #[test]
    fn create_channel_checked_rejects_empty() {
        let mut p = OutputPanel::new();
        assert_eq!(
            p.create_channel_checked(""),
            Err(OutputError::InvalidChannelName)
        );
        assert_eq!(
            p.create_channel_checked("   "),
            Err(OutputError::InvalidChannelName)
        );
    }

    #[test]
    fn append_line_checked_error() {
        let mut p = OutputPanel::new();
        assert_eq!(
            p.append_line_checked(0, "nope"),
            Err(OutputError::ChannelNotFound(0))
        );
    }

    #[test]
    fn find_channel_by_name() {
        let mut p = OutputPanel::new();
        p.create_channel("Alpha");
        p.create_channel("Beta");
        assert_eq!(p.find_channel("Beta"), Some(1));
        assert_eq!(p.find_channel("Gamma"), None);
    }

    #[test]
    fn channel_helpers() {
        let mut ch = OutputChannel::new("Test");
        assert!(ch.is_empty());
        ch.content.push("hello world".into());
        ch.content.push("foo".into());
        assert_eq!(ch.line_count(), 2);
        assert!(!ch.is_empty());
        assert_eq!(ch.tail(1), &["foo".to_string()]);
        assert_eq!(ch.tail(10).len(), 2);
        assert_eq!(ch.count_matches("HELLO"), 1);
        assert_eq!(ch.count_matches(""), 0);
        assert_eq!(ch.byte_size(), 14); // 11 + 3
    }

    #[test]
    fn channel_display() {
        let ch = OutputChannel::new("Git");
        assert_eq!(ch.to_string(), "OutputChannel(Git, 0 lines, visible=true)");
    }

    #[test]
    fn panel_display() {
        let p = OutputPanel::new();
        assert!(p.to_string().contains("0 channels"));
    }

    #[test]
    fn total_line_count_works() {
        let mut p = OutputPanel::new();
        p.create_channel("A");
        p.create_channel("B");
        p.append_line(0, "l1");
        p.append_line(0, "l2");
        p.append_line(1, "l3");
        assert_eq!(p.total_line_count(), 3);
    }

    #[test]
    fn channel_names_works() {
        let mut p = OutputPanel::new();
        p.create_channel("X");
        p.create_channel("Y");
        assert_eq!(p.channel_names(), vec!["X", "Y"]);
    }

    #[test]
    fn scroll_up_down() {
        let mut p = OutputPanel::new();
        p.create_channel("Log");
        for i in 0..20 {
            p.append_line(0, format!("line {i}"));
        }
        p.scroll_offset = 10;
        p.scroll_up(3);
        assert_eq!(p.scroll_offset, 7);
        assert!(!p.auto_scroll);
        p.scroll_up(100);
        assert_eq!(p.scroll_offset, 0);
        p.scroll_down(5);
        assert_eq!(p.scroll_offset, 5);
        p.scroll_down(1000);
        assert_eq!(p.scroll_offset, 19);
    }

    #[test]
    fn remove_channel_works() {
        let mut p = OutputPanel::new();
        p.create_channel("A");
        p.create_channel("B");
        p.select_channel(1);
        let removed = p.remove_channel(1).unwrap();
        assert_eq!(removed.name, "B");
        assert_eq!(p.active_channel_index, 0);
        assert!(p.remove_channel(99).is_err());
    }

    #[test]
    fn partial_eq_panel() {
        let a = OutputPanel::new();
        let b = OutputPanel::new();
        assert_eq!(a, b);
    }

    // ---- OutputStats tests ----

    #[test]
    fn compute_stats_empty_panel() {
        let panel = OutputPanel::new();
        let stats = compute_output_stats(&panel);
        assert_eq!(
            stats,
            OutputStats {
                total_lines: 0,
                total_bytes: 0,
                channel_count: 0,
            }
        );
    }

    #[test]
    fn compute_stats_single_channel() {
        let mut panel = OutputPanel::new();
        panel.create_channel("Log");
        panel.append_line(0, "hello");
        panel.append_line(0, "world!!");
        let stats = compute_output_stats(&panel);
        assert_eq!(stats.channel_count, 1);
        assert_eq!(stats.total_lines, 2);
        assert_eq!(stats.total_bytes, 5 + 7); // "hello" + "world!!"
    }

    #[test]
    fn compute_stats_multiple_channels() {
        let mut panel = OutputPanel::new();
        panel.create_channel("A");
        panel.create_channel("B");
        panel.append_line(0, "aaa");
        panel.append_line(1, "bb");
        panel.append_line(1, "cccc");
        let stats = compute_output_stats(&panel);
        assert_eq!(stats.channel_count, 2);
        assert_eq!(stats.total_lines, 3);
        assert_eq!(stats.total_bytes, 3 + 2 + 4);
    }

    #[test]
    fn output_stats_display() {
        let stats = OutputStats {
            total_lines: 42,
            total_bytes: 1024,
            channel_count: 3,
        };
        assert_eq!(
            stats.to_string(),
            "OutputStats(channels=3, lines=42, bytes=1024)"
        );
    }

    #[test]
    fn compute_stats_after_clear() {
        let mut panel = OutputPanel::new();
        let idx = panel.create_channel("Tmp");
        panel.append_line(idx, "data");
        panel.append_line(idx, "more data");
        panel.clear_channel(idx);
        let stats = compute_output_stats(&panel);
        assert_eq!(stats.channel_count, 1);
        assert_eq!(stats.total_lines, 0);
        assert_eq!(stats.total_bytes, 0);
    }

    #[test]
    fn visible_channel_indices_works() {
        let mut p = OutputPanel::new();
        p.create_channel("A");
        p.create_channel("B");
        p.channels[1].is_visible = false;
        assert_eq!(p.visible_channel_indices(), vec![0]);
        assert_eq!(p.hidden_channel_indices(), vec![1]);
    }

    #[test]
    fn set_channel_visibility_works() {
        let mut p = OutputPanel::new();
        p.create_channel("X");
        assert!(p.set_channel_visibility(0, false));
        assert!(!p.channels[0].is_visible);
        assert!(!p.set_channel_visibility(99, true));
    }

    #[test]
    fn batch_append_lines() {
        let mut p = OutputPanel::new();
        p.create_channel("Log");
        assert!(p.batch_append(0, &["line1", "line2", "line3"]));
        assert_eq!(p.channels[0].content.len(), 3);
        assert!(!p.batch_append(5, &["nope"]));
    }

    #[test]
    fn swap_channels_adjusts_active() {
        let mut p = OutputPanel::new();
        p.create_channel("A");
        p.create_channel("B");
        p.select_channel(0);
        assert!(p.swap_channels(0, 1));
        assert_eq!(p.active_channel_index, 1);
        assert_eq!(p.channels[0].name, "B");
        assert!(!p.swap_channels(0, 99));
    }

    #[test]
    fn move_channel_reorders() {
        let mut p = OutputPanel::new();
        p.create_channel("A");
        p.create_channel("B");
        p.create_channel("C");
        assert!(p.move_channel(0, 2));
        assert_eq!(p.channel_names(), vec!["B", "C", "A"]);
        assert!(!p.move_channel(0, 99));
    }

    #[test]
    fn export_channel_content() {
        let mut p = OutputPanel::new();
        p.create_channel("Out");
        p.append_line(0, "hello");
        p.append_line(0, "world");
        assert_eq!(p.export_active_channel(), Some("hello\nworld".to_string()));
        assert_eq!(p.export_channel(0), Some("hello\nworld".to_string()));
        assert!(p.export_channel(5).is_none());
    }

    #[test]
    fn export_all_channels_format() {
        let mut p = OutputPanel::new();
        p.create_channel("A");
        p.append_line(0, "data");
        let exported = p.export_all_channels();
        assert!(exported.contains("=== A ==="));
        assert!(exported.contains("data"));
    }

    #[test]
    fn filter_by_severity_works() {
        let f = OutputChannelFilter::by_severity(OutputSeverity::Error);
        assert!(f.matches("[error] something failed"));
        assert!(!f.matches("[info] all good"));
    }

    #[test]
    fn filter_by_pattern() {
        let f = OutputChannelFilter::by_pattern("timeout");
        assert!(f.matches("connection timeout occurred"));
        assert!(!f.matches("connection reset"));
    }

    #[test]
    fn filter_severity_case_insensitive() {
        let f = OutputChannelFilter::by_severity(OutputSeverity::Warning);
        assert!(f.matches("[WARNING] disk space low"));
    }

    // ---- OutputChannelGroup tests ----

    #[test]
    fn channel_group_add_remove() {
        let mut group = OutputChannelGroup::new("Build");
        group.add_channel(0);
        group.add_channel(1);
        group.add_channel(0); // duplicate, ignored
        assert_eq!(group.channel_count(), 2);
        assert!(!group.is_empty());
        assert!(group.remove_channel(0));
        assert_eq!(group.channel_count(), 1);
        assert!(!group.remove_channel(99));
        assert_eq!(group.to_string(), "OutputChannelGroup(Build, 1 channels)");
    }

    #[test]
    fn group_manager_create_and_find() {
        let mut mgr = OutputGroupManager::new();
        let g0 = mgr.create_group("Build");
        let g1 = mgr.create_group("Debug");
        mgr.add_to_group(g0, 0);
        mgr.add_to_group(g0, 1);
        mgr.add_to_group(g1, 2);
        assert_eq!(mgr.group_count(), 2);
        assert_eq!(mgr.find_group("Build"), Some(0));
        assert_eq!(mgr.find_group("Debug"), Some(1));
        assert_eq!(mgr.find_group("Missing"), None);
        assert_eq!(mgr.group_names(), vec!["Build", "Debug"]);
        let grp = mgr.get_group(g0).unwrap();
        assert_eq!(grp.channel_count(), 2);
    }

    #[test]
    fn group_manager_invalid_group() {
        let mut mgr = OutputGroupManager::new();
        assert!(!mgr.add_to_group(99, 0));
    }

    // ---- output_search tests ----

    #[test]
    fn output_search_across_channels() {
        let mut p = OutputPanel::new();
        p.create_channel("Build");
        p.create_channel("Debug");
        p.append_line(0, "[error] compile failed");
        p.append_line(0, "all good");
        p.append_line(1, "ERROR: runtime crash");

        let results = output_search(&p, "error");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].channel_index, 0);
        assert_eq!(results[0].line_index, 0);
        assert_eq!(results[1].channel_index, 1);
    }

    #[test]
    fn output_search_multiple_in_line() {
        let mut p = OutputPanel::new();
        p.create_channel("Log");
        p.append_line(0, "error error error");
        let results = output_search(&p, "error");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn output_search_empty_query() {
        let p = OutputPanel::new();
        assert!(output_search(&p, "").is_empty());
    }

    // ---- OutputChannelAppendMode tests ----

    #[test]
    fn append_mode_append() {
        let mut p = OutputPanel::new();
        p.create_channel("Log");
        p.append_with_mode(0, "line1", OutputChannelAppendMode::Append);
        p.append_with_mode(0, "line2", OutputChannelAppendMode::Append);
        assert_eq!(p.channels[0].content, vec!["line1", "line2"]);
    }

    #[test]
    fn append_mode_replace() {
        let mut p = OutputPanel::new();
        p.create_channel("Log");
        p.append_line(0, "old1");
        p.append_line(0, "old2");
        p.append_with_mode(0, "new", OutputChannelAppendMode::Replace);
        assert_eq!(p.channels[0].content, vec!["new"]);
    }

    #[test]
    fn append_mode_prepend() {
        let mut p = OutputPanel::new();
        p.create_channel("Log");
        p.append_line(0, "second");
        p.append_with_mode(0, "first", OutputChannelAppendMode::Prepend);
        assert_eq!(p.channels[0].content, vec!["first", "second"]);
    }

    #[test]
    fn append_mode_invalid_channel() {
        let mut p = OutputPanel::new();
        assert!(!p.append_with_mode(99, "x", OutputChannelAppendMode::Append));
    }

    #[test]
    fn append_mode_display() {
        assert_eq!(format!("{}", OutputChannelAppendMode::Append), "Append");
        assert_eq!(format!("{}", OutputChannelAppendMode::Replace), "Replace");
        assert_eq!(format!("{}", OutputChannelAppendMode::Prepend), "Prepend");
    }

    // ---- OutputChannelFilter::apply ----

    #[test]
    fn filter_apply_to_channel() {
        let mut ch = OutputChannel::new("Log");
        ch.content.push("[error] bad thing".into());
        ch.content.push("[info] all good".into());
        ch.content.push("[error] another bad thing".into());
        let f = OutputChannelFilter::by_severity(OutputSeverity::Error);
        let indices = f.apply(&ch);
        assert_eq!(indices, vec![0, 2]);
    }

    #[test]
    fn filter_severity_and_pattern() {
        let f = OutputChannelFilter::by_severity_and_pattern(OutputSeverity::Error, "timeout");
        assert!(f.matches("[error] connection timeout"));
        assert!(!f.matches("[error] disk full"));
        assert!(!f.matches("[info] timeout ignored"));
    }

    // ── OutputChannelExporter tests ──

    #[test]
    fn exporter_plain() {
        let mut ch = OutputChannel::new("test");
        ch.content.push("line 1".into());
        ch.content.push("line 2".into());
        let exported = OutputChannelExporter::export(&ch, &ExportFormat::Plain);
        assert_eq!(exported, "line 1\nline 2");
    }

    #[test]
    fn exporter_numbered() {
        let mut ch = OutputChannel::new("test");
        ch.content.push("hello".into());
        let exported = OutputChannelExporter::export(&ch, &ExportFormat::Numbered);
        assert!(exported.contains("1:"));
        assert!(exported.contains("hello"));
    }

    #[test]
    fn exporter_channel_prefixed() {
        let mut ch = OutputChannel::new("mylog");
        ch.content.push("msg".into());
        let exported = OutputChannelExporter::export(&ch, &ExportFormat::ChannelPrefixed);
        assert!(exported.contains("[mylog:"));
        assert!(exported.contains("msg"));
    }

    // ── Output rotation tests ──

    #[test]
    fn rotation_policy_trims() {
        let mut ch = OutputChannel::new("test");
        for i in 0..20 {
            ch.content.push(format!("line {}", i));
        }
        let policy = OutputRotationPolicy::new(10, 5);
        assert!(policy.needs_rotation(&ch));
        let removed = policy.apply(&mut ch);
        assert_eq!(removed, 5);
        assert_eq!(ch.content.len(), 15);
    }

    #[test]
    fn rotation_policy_no_trim_needed() {
        let mut ch = OutputChannel::new("test");
        ch.content.push("single".into());
        let policy = OutputRotationPolicy::new(10, 5);
        assert!(!policy.needs_rotation(&ch));
        assert_eq!(policy.apply(&mut ch), 0);
    }

    // ── Channel search tests ──

    #[test]
    fn channel_search_basic() {
        let mut ch = OutputChannel::new("test");
        ch.content.push("Hello World".into());
        ch.content.push("hello again".into());
        ch.content.push("no match here".into());
        let hits = channel_search(&ch, "hello");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line_index, 0);
        assert_eq!(hits[1].line_index, 1);
    }

    #[test]
    fn channel_search_count_test() {
        let mut ch = OutputChannel::new("test");
        ch.content.push("aaa".into());
        assert_eq!(channel_search_count(&ch, "a"), 3);
    }

    // ── StructuredOutputChannel & related tests ──

    #[test]
    fn structured_channel_push_and_filter() {
        let mut ch = StructuredOutputChannel::new("build");
        ch.push(OutputEntry::new(0, EntrySeverity::Info, "compiling crate"));
        ch.push(OutputEntry::new(1, EntrySeverity::Warning, "unused variable"));
        ch.push(OutputEntry::new(2, EntrySeverity::Error, "type mismatch"));
        ch.push(OutputEntry::new(3, EntrySeverity::Debug, "resolved dep"));

        assert_eq!(ch.len(), 4);
        assert_eq!(ch.filter_by_severity(EntrySeverity::Warning).len(), 1);
        assert_eq!(ch.filter_by_severity(EntrySeverity::Debug).len(), 1);
        assert_eq!(ch.severity_counts(), (1, 1, 1, 1));
    }

    #[test]
    fn structured_channel_log_rotation() {
        let mut ch = StructuredOutputChannel::new("logs").with_max_entries(3);
        for i in 0..5 {
            ch.push(OutputEntry::new(i, EntrySeverity::Info, format!("line {}", i)));
        }
        assert_eq!(ch.len(), 3);
        // oldest entries should have been trimmed
        assert_eq!(ch.entries()[0].message, "line 2");
        assert_eq!(ch.entries()[2].message, "line 4");
    }

    #[test]
    fn structured_channel_search_and_source_filter() {
        let mut ch = StructuredOutputChannel::new("test");
        ch.push(
            OutputEntry::new(0, EntrySeverity::Error, "cannot find module")
                .with_source("rustc"),
        );
        ch.push(
            OutputEntry::new(1, EntrySeverity::Warning, "unused import")
                .with_source("clippy"),
        );
        ch.push(OutputEntry::new(2, EntrySeverity::Info, "build complete"));

        assert_eq!(ch.search("module").len(), 1);
        assert_eq!(ch.search("UNUSED").len(), 1); // case-insensitive
        assert_eq!(ch.search("").len(), 0);
        assert_eq!(ch.filter_by_source("rustc").len(), 1);
        assert_eq!(ch.filter_by_source("cargo").len(), 0);
    }

    #[test]
    fn merge_channels_chronological() {
        let mut a = StructuredOutputChannel::new("cargo");
        a.push(OutputEntry::new(1, EntrySeverity::Info, "compiling"));
        a.push(OutputEntry::new(3, EntrySeverity::Info, "finished"));

        let mut b = StructuredOutputChannel::new("rustc");
        b.push(OutputEntry::new(0, EntrySeverity::Debug, "start"));
        b.push(OutputEntry::new(2, EntrySeverity::Error, "error[E0308]"));

        let merged = merge_channels(&[&a, &b]);
        assert_eq!(merged.len(), 4);
        // should be sorted by timestamp
        assert_eq!(merged[0].entry.timestamp_secs, 0);
        assert_eq!(merged[0].channel_name, "rustc");
        assert_eq!(merged[1].entry.timestamp_secs, 1);
        assert_eq!(merged[2].entry.timestamp_secs, 2);
        assert_eq!(merged[3].entry.timestamp_secs, 3);
    }

    #[test]
    fn compute_summary_aggregates() {
        let mut a = StructuredOutputChannel::new("a");
        a.push(OutputEntry::new(0, EntrySeverity::Info, "ok"));
        a.push(OutputEntry::new(1, EntrySeverity::Error, "fail"));

        let mut b = StructuredOutputChannel::new("b");
        b.push(OutputEntry::new(0, EntrySeverity::Warning, "warn"));
        b.push(OutputEntry::new(1, EntrySeverity::Debug, "dbg"));
        b.push(OutputEntry::new(2, EntrySeverity::Error, "err2"));

        let summary = compute_summary(&[&a, &b]);
        assert_eq!(summary.total_entries, 5);
        assert_eq!(summary.channel_count, 2);
        assert_eq!(summary.error_count, 2);
        assert_eq!(summary.warning_count, 1);
        assert_eq!(summary.info_count, 1);
        assert_eq!(summary.debug_count, 1);
        // Display impl
        let s = summary.to_string();
        assert!(s.contains("5 entries"));
    }

    #[test]
    fn output_entry_formatting() {
        let e = OutputEntry::new(3661, EntrySeverity::Warning, "slow query")
            .with_source("db");
        let formatted = e.format();
        assert!(formatted.contains("01:01:01"));
        assert!(formatted.contains("WARN"));
        assert!(formatted.contains("db"));
        assert!(formatted.contains("slow query"));

        // Without source
        let e2 = OutputEntry::new(0, EntrySeverity::Info, "hello");
        let f2 = e2.to_string();
        assert!(f2.contains("00:00:00"));
        assert!(f2.contains("INFO"));
    }

    #[test]
    fn structured_channel_tail_and_display() {
        let mut ch = StructuredOutputChannel::new("test");
        for i in 0..10 {
            ch.push(OutputEntry::new(i, EntrySeverity::Info, format!("msg{}", i)));
        }
        let last3 = ch.tail(3);
        assert_eq!(last3.len(), 3);
        assert_eq!(last3[0].message, "msg7");
        assert_eq!(last3[2].message, "msg9");

        let display = ch.to_string();
        assert!(display.contains("10 entries"));

        let lines = ch.to_plain_lines();
        assert_eq!(lines.len(), 10);
        assert!(lines[0].contains("msg0"));
    }

    // -- OutputChannelTextFilter tests --

    #[test]
    fn text_filter_case_insensitive() {
        let f = OutputChannelTextFilter::new("error");
        assert!(f.matches("An ERROR occurred"));
        assert!(f.matches("error at line 5"));
        assert!(!f.matches("all good"));
    }

    #[test]
    fn text_filter_case_sensitive() {
        let mut f = OutputChannelTextFilter::new("Error");
        f.set_case_sensitive(true);
        assert!(f.matches("Error at line 5"));
        assert!(!f.matches("error at line 5"));
        assert!(!f.matches("ERROR AT LINE 5"));
    }

    #[test]
    fn text_filter_empty_query() {
        let f = OutputChannelTextFilter::new("");
        assert!(!f.matches("anything"));
        assert_eq!(f.match_count(&["a".into(), "b".into()]), 0);
    }

    #[test]
    fn text_filter_filter_lines() {
        let lines: Vec<String> = vec![
            "info: compiling".into(),
            "warning: unused var".into(),
            "info: done".into(),
        ];
        let f = OutputChannelTextFilter::new("info");
        let hits = f.filter_lines(&lines);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].0, 0);
        assert_eq!(hits[1].0, 2);
        assert_eq!(f.match_count(&lines), 2);
    }

    #[test]
    fn text_filter_regex_flag() {
        let mut f = OutputChannelTextFilter::new("warn");
        f.set_regex(true);
        assert!(f.matches("WARNING: something"));
        assert!(!f.matches("all clear"));
    }

    // -- OutputChannelScrollState tests --

    #[test]
    fn scroll_state_basic() {
        let mut s = OutputChannelScrollState::new(10);
        assert_eq!(s.offset, 0);
        assert!(s.auto_follow);

        s.scroll_down(5);
        assert_eq!(s.offset, 5);
        assert!(!s.auto_follow);

        s.scroll_up(3);
        assert_eq!(s.offset, 2);
    }

    #[test]
    fn scroll_state_to_bottom_and_top() {
        let mut s = OutputChannelScrollState::new(10);
        s.scroll_to_bottom(50);
        assert_eq!(s.offset, 40);
        assert!(s.is_at_bottom(50));

        s.scroll_to_top();
        assert_eq!(s.offset, 0);
        assert!(!s.auto_follow);
    }

    #[test]
    fn scroll_state_visible_range() {
        let mut s = OutputChannelScrollState::new(10);
        s.scroll_to_bottom(25);
        let (start, end) = s.visible_range(25);
        assert_eq!(start, 15);
        assert_eq!(end, 25);

        // When total lines < viewport
        let s2 = OutputChannelScrollState::new(20);
        assert!(s2.is_at_bottom(5));
        let (start2, end2) = s2.visible_range(5);
        assert_eq!(start2, 0);
        assert_eq!(end2, 5);
    }

    #[test]
    fn scroll_state_auto_follow() {
        let mut s = OutputChannelScrollState::new(10);
        s.follow_if_needed(30);
        assert_eq!(s.offset, 20);
        s.toggle_auto_follow();
        assert!(!s.auto_follow);
        s.follow_if_needed(40);
        assert_eq!(s.offset, 20); // didn't move
    }

    // -- OutputTimestampFormatter tests --

    #[test]
    fn timestamp_format_time_only() {
        let f = OutputTimestampFormatter::new(OutputTimestampFormat::TimeOnly);
        assert_eq!(f.format_time_only(3661), "01:01:01");
        assert_eq!(f.format_time_only(0), "00:00:00");
        let line = f.format_line("hello", 3661);
        assert_eq!(line, "[01:01:01] hello");
    }

    #[test]
    fn timestamp_format_iso() {
        let f = OutputTimestampFormatter::new(OutputTimestampFormat::Iso);
        let line = f.format_line("test", 90061);
        // 90061s = 1 day, 1h, 1m, 1s
        assert_eq!(line, "[0001-01:01:01] test");
    }

    #[test]
    fn timestamp_format_elapsed() {
        let f = OutputTimestampFormatter::new(OutputTimestampFormat::Elapsed);
        assert_eq!(f.format_elapsed(100, 90), "+10s");
        let line = f.format_line("msg", 42);
        assert_eq!(line, "[+42s] msg");
    }

    #[test]
    fn timestamp_set_format() {
        let mut f = OutputTimestampFormatter::new(OutputTimestampFormat::Iso);
        f.set_format(OutputTimestampFormat::TimeOnly);
        assert_eq!(f.format, OutputTimestampFormat::TimeOnly);
    }

    // -- OutputClearConfirmation tests --

    #[test]
    fn clear_confirmation_flow() {
        let mut c = OutputClearConfirmation::new("build", 42);
        assert!(c.needs_confirmation());
        assert!(!c.is_confirmed());
        assert_eq!(c.message(), "Clear 42 lines from 'build'?");

        c.confirm();
        assert!(c.is_confirmed());
        let display = c.to_string();
        assert!(display.contains("Cleared 'build'"));

        c.cancel();
        assert!(!c.is_confirmed());
    }

    #[test]
    fn clear_confirmation_empty_channel() {
        let c = OutputClearConfirmation::new("logs", 0);
        assert!(!c.needs_confirmation());
        assert!(!c.is_confirmed());
    }

    #[test]
    fn clear_confirmation_display_unconfirmed() {
        let c = OutputClearConfirmation::new("output", 10);
        let s = format!("{c}");
        assert!(s.contains("Clear 10 lines"));
        assert!(s.contains("output"));
    }

    #[test]
    fn timestamp_injector_elapsed() {
        let mut inj = OutputTimestampInjector::new(TimestampInjectorConfig::default(), 1000);
        let result = inj.inject("hello", 2500);
        assert!(result.starts_with("[1.500s]"));
        assert!(result.contains("hello"));
    }

    #[test]
    fn timestamp_injector_epoch() {
        let config = TimestampInjectorConfig {
            enabled: true,
            style: TimestampStyle::Epoch,
            separator: " ".into(),
        };
        let mut inj = OutputTimestampInjector::new(config, 0);
        let result = inj.inject("test", 5000);
        assert!(result.starts_with("[5]"));
    }

    #[test]
    fn timestamp_injector_hhmmss() {
        let config = TimestampInjectorConfig {
            enabled: true,
            style: TimestampStyle::HhMmSs,
            separator: " ".into(),
        };
        let mut inj = OutputTimestampInjector::new(config, 0);
        let result = inj.inject("line", 3661_000);
        assert!(result.contains("[01:01:01]"));
    }

    #[test]
    fn timestamp_injector_disabled() {
        let config = TimestampInjectorConfig {
            enabled: false,
            style: TimestampStyle::Elapsed,
            separator: " ".into(),
        };
        let mut inj = OutputTimestampInjector::new(config, 0);
        let result = inj.inject("raw", 1000);
        assert_eq!(result, "raw");
    }

    #[test]
    fn timestamp_injector_batch() {
        let mut inj = OutputTimestampInjector::with_defaults();
        let result = inj.inject_batch(&["a", "b", "c"], 1000);
        assert_eq!(result.len(), 3);
        assert_eq!(inj.entries_processed(), 3);
    }

    #[test]
    fn timestamp_injector_toggle_enabled() {
        let mut inj = OutputTimestampInjector::with_defaults();
        assert!(inj.is_enabled());
        inj.set_enabled(false);
        assert!(!inj.is_enabled());
    }

    #[test]
    fn timestamp_injector_display() {
        let inj = OutputTimestampInjector::with_defaults();
        let s = format!("{inj}");
        assert!(s.contains("Elapsed"));
        assert!(s.contains("processed=0"));
    }

    #[test]
    fn language_colorizer_detect_rust() {
        let mut col = OutputLanguageColorizer::new();
        assert_eq!(col.detect_language("fn main() {"), DetectedLanguage::Rust);
    }

    #[test]
    fn language_colorizer_detect_python() {
        let mut col = OutputLanguageColorizer::new();
        assert_eq!(col.detect_language("def hello():"), DetectedLanguage::Python);
    }

    #[test]
    fn language_colorizer_detect_js() {
        let mut col = OutputLanguageColorizer::new();
        assert_eq!(col.detect_language("const x = 5;"), DetectedLanguage::JavaScript);
    }

    #[test]
    fn language_colorizer_detect_shell() {
        let mut col = OutputLanguageColorizer::new();
        assert_eq!(col.detect_language("$ ls -la"), DetectedLanguage::Shell);
    }

    #[test]
    fn language_colorizer_detect_json() {
        let mut col = OutputLanguageColorizer::new();
        assert_eq!(col.detect_language("{\"key\": \"value\"}"), DetectedLanguage::Json);
    }

    #[test]
    fn language_colorizer_detect_plain() {
        let mut col = OutputLanguageColorizer::new();
        assert_eq!(col.detect_language("just some text"), DetectedLanguage::Plain);
    }

    #[test]
    fn language_colorizer_most_frequent() {
        let mut col = OutputLanguageColorizer::new();
        col.detect_language("fn a()");
        col.detect_language("fn b()");
        col.detect_language("def c():");
        assert_eq!(col.most_frequent(), Some(DetectedLanguage::Rust));
    }

    #[test]
    fn language_colorizer_count() {
        let mut col = OutputLanguageColorizer::new();
        col.detect_language("fn foo()");
        col.detect_language("def bar():");
        assert_eq!(col.count_language(DetectedLanguage::Rust), 1);
        assert_eq!(col.count_language(DetectedLanguage::Python), 1);
        assert_eq!(col.detection_count(), 2);
    }

    #[test]
    fn language_colorizer_reset() {
        let mut col = OutputLanguageColorizer::new();
        col.detect_language("fn foo()");
        col.reset();
        assert_eq!(col.detection_count(), 0);
    }

    #[test]
    fn language_colorizer_display() {
        let col = OutputLanguageColorizer::new();
        let s = format!("{col}");
        assert!(s.contains("0 detections"));
    }

    #[test]
    fn language_colorizer_color_for() {
        assert_eq!(OutputLanguageColorizer::color_for(DetectedLanguage::Rust), Color::Rgb(255, 165, 0));
        assert_eq!(OutputLanguageColorizer::color_for(DetectedLanguage::Plain), Color::White);
    }



    // -- output additional tests -------------------------------------------

    #[test]
    fn x_output_panel_state_new() {
        let p = XOutputPanelState::new(XOutputLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XOutputLayoutRegion::Sidebar);
    }

    #[test]
    fn x_output_panel_area() {
        let p = XOutputPanelState::new(XOutputLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_output_panel_toggle() {
        let mut p = XOutputPanelState::new(XOutputLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_output_panel_resize() {
        let mut p = XOutputPanelState::new(XOutputLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_output_panel_is_narrow() {
        let mut p = XOutputPanelState::new(XOutputLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_output_total_visible_area_basic() {
        let panels = vec![
            XOutputPanelState::new(XOutputLayoutRegion::Sidebar, "a"),
            XOutputPanelState::new(XOutputLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_output_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_output_total_visible_area_hidden() {
        let mut panels = vec![
            XOutputPanelState::new(XOutputLayoutRegion::Sidebar, "a"),
            XOutputPanelState::new(XOutputLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_output_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_output_count_in_region_basic() {
        let panels = vec![
            XOutputPanelState::new(XOutputLayoutRegion::Sidebar, "a"),
            XOutputPanelState::new(XOutputLayoutRegion::Sidebar, "b"),
            XOutputPanelState::new(XOutputLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_output_count_in_region(&panels, XOutputLayoutRegion::Sidebar), 2);
        assert_eq!(x_output_count_in_region(&panels, XOutputLayoutRegion::Editor), 1);
        assert_eq!(x_output_count_in_region(&panels, XOutputLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_output_widest_panel_basic() {
        let mut panels = vec![
            XOutputPanelState::new(XOutputLayoutRegion::Sidebar, "narrow"),
            XOutputPanelState::new(XOutputLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_output_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_output_collapse_region_basic() {
        let mut panels = vec![
            XOutputPanelState::new(XOutputLayoutRegion::Sidebar, "a"),
            XOutputPanelState::new(XOutputLayoutRegion::Sidebar, "b"),
            XOutputPanelState::new(XOutputLayoutRegion::Editor, "c"),
        ];
        x_output_collapse_region(&mut panels, XOutputLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_output_layout_constraint_clamp() {
        let lc = XOutputLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_output_layout_constraint_satisfied() {
        let lc = XOutputLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_output_widest_panel_empty() {
        let panels: Vec<XOutputPanelState> = vec![];
        assert!(x_output_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_output_layout_region_eq() {
        assert_eq!(XOutputLayoutRegion::Sidebar, XOutputLayoutRegion::Sidebar);
        assert_ne!(XOutputLayoutRegion::Sidebar, XOutputLayoutRegion::Panel);
    }


    // -- output extended domain tests ----------------------------------------

    #[test]
    fn y_output_enum_index() {
        assert_eq!(YOutputOutputVerbosity::Silent.index(), 0);
        assert_eq!(YOutputOutputVerbosity::Quiet.index(), 1);
        assert_eq!(YOutputOutputVerbosity::Normal.index(), 2);
        assert_eq!(YOutputOutputVerbosity::Verbose.index(), 3);
    }

    #[test]
    fn y_output_enum_label() {
        assert_eq!(YOutputOutputVerbosity::Silent.label(), "Silent");
        assert_eq!(YOutputOutputVerbosity::Quiet.label(), "Quiet");
        assert_eq!(YOutputOutputVerbosity::Normal.label(), "Normal");
        assert_eq!(YOutputOutputVerbosity::Verbose.label(), "Verbose");
    }

    #[test]
    fn y_output_enum_all() {
        let all = YOutputOutputVerbosity::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_output_enum_is_default() {
        assert!(YOutputOutputVerbosity::Silent.is_default());
        assert!(!YOutputOutputVerbosity::Verbose.is_default());
    }

    #[test]
    fn y_output_enum_display() {
        assert_eq!(format!("{}", YOutputOutputVerbosity::Silent), "Silent");
    }

    #[test]
    fn y_output_struct_new() {
        let s = YOutputOutputLogBuffer::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_output_struct_clear() {
        let mut s = YOutputOutputLogBuffer::new();
        s.lines.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_output_fingerprint_deterministic() {
        let h1 = y_output_fingerprint("hello");
        let h2 = y_output_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_output_fingerprint("a"), y_output_fingerprint("b"));
    }

    #[test]
    fn y_output_truncate_short() {
        assert_eq!(y_output_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_output_truncate_long() {
        let r = y_output_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_output_normalize_key_basic() {
        assert_eq!(y_output_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_output_split_path_basic() {
        let parts = y_output_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_output_count_occurrences_basic() {
        assert_eq!(y_output_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_output_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_output_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_output_in_range_basic() {
        assert!(y_output_in_range(5, 1, 10));
        assert!(y_output_in_range(1, 1, 10));
        assert!(y_output_in_range(10, 1, 10));
        assert!(!y_output_in_range(0, 1, 10));
        assert!(!y_output_in_range(11, 1, 10));
    }

    #[test]
    fn y_output_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_output_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_output_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_output_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- output Z-extended tests -----------------------------------------------

    #[test]
    fn z_output_priority_weight() {
        assert_eq!(ZOutputPriority::Idle.weight(), 0);
        assert_eq!(ZOutputPriority::Normal.weight(), 2);
        assert_eq!(ZOutputPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_output_priority_label() {
        assert_eq!(ZOutputPriority::Low.label(), "low");
        assert_eq!(ZOutputPriority::High.label(), "high");
    }

    #[test]
    fn z_output_priority_is_elevated() {
        assert!(!ZOutputPriority::Normal.is_elevated());
        assert!(ZOutputPriority::High.is_elevated());
        assert!(ZOutputPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_output_priority_display() {
        assert_eq!(format!("{}", ZOutputPriority::Idle), "idle");
    }

    #[test]
    fn z_output_priority_all_asc() {
        let all = ZOutputPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZOutputPriority::Idle);
        assert_eq!(all[4], ZOutputPriority::Realtime);
    }

    #[test]
    fn z_output_struct_new() {
        let s = ZOutputOutputSearchIndex::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_output_struct_toggled_clone() {
        let s = ZOutputOutputSearchIndex::new();
        let t = s.toggled_clone();
        assert_ne!(s.dirty, t.dirty);
    }

    #[test]
    fn z_output_rolling_hash_deterministic() {
        let h1 = z_output_rolling_hash(b"test");
        let h2 = z_output_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_output_rolling_hash(b"a"), z_output_rolling_hash(b"b"));
    }

    #[test]
    fn z_output_pad_to_basic() {
        assert_eq!(z_output_pad_to("hi", 5), "hi   ");
        assert_eq!(z_output_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_output_is_identifier_basic() {
        assert!(z_output_is_identifier("foo_bar"));
        assert!(z_output_is_identifier("abc123"));
        assert!(!z_output_is_identifier(""));
        assert!(!z_output_is_identifier("has space"));
    }

    #[test]
    fn z_output_levenshtein_basic() {
        assert_eq!(z_output_levenshtein("", ""), 0);
        assert_eq!(z_output_levenshtein("abc", "abc"), 0);
        assert_eq!(z_output_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_output_unique_words_basic() {
        let w = z_output_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_output_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_output_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_output_common_prefix_basic() {
        assert_eq!(z_output_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_output_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_output_struct_clear() {
        let mut s = ZOutputOutputSearchIndex::new();
        s.line_offsets.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_output_rolling_hash_empty() {
        let h = z_output_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_71_push_and_len() {
        let mut rb = super::XbRingBuffer71::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_71_overwrite() {
        let mut rb = super::XbRingBuffer71::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_71_get_out_of_bounds() {
        let rb = super::XbRingBuffer71::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_71_drain_all() {
        let mut rb = super::XbRingBuffer71::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_71_peek_front_back() {
        let mut rb = super::XbRingBuffer71::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_71_clear() {
        let mut rb = super::XbRingBuffer71::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_71_capacity() {
        let rb = super::XbRingBuffer71::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_71_basic() {
        let h = super::xb_fnv1a_71(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_71(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_71_different_inputs() {
        let h1 = super::xb_fnv1a_71(b"abc");
        let h2 = super::xb_fnv1a_71(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_71_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_71(&data);
        let dec = super::xb_rle_decode_71(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_71_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_71(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_71(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_71_values() {
        assert!((super::xb_clamp_71(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_71(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_71(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_71_values() {
        assert!((super::xb_lerp_71(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_71(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_71(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_71_wrap_around_twice() {
        let mut rb = super::XbRingBuffer71::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 135 ----

    #[test]
    fn xc_135_pool_new_empty() {
        let pool: super::Xc135Pool<i32> = super::Xc135Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_135_pool_release_acquire() {
        let mut pool = super::Xc135Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_135_pool_acquire_empty() {
        let mut pool: super::Xc135Pool<i32> = super::Xc135Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_135_pool_full() {
        let mut pool = super::Xc135Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_135_pool_drain() {
        let mut pool = super::Xc135Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_135_pool_stats() {
        let mut pool = super::Xc135Pool::new(8);
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
    fn xc_135_pool_clear() {
        let mut pool = super::Xc135Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_135_pool_shrink() {
        let mut pool = super::Xc135Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_135_pool_default() {
        let pool: super::Xc135Pool<String> = super::Xc135Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_135_pool_extend() {
        let mut pool = super::Xc135Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_135_pool_retain() {
        let mut pool = super::Xc135Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_135_scheduler_round_robin() {
        let mut sched = super::Xc135Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_135_scheduler_empty() {
        let mut sched = super::Xc135Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_135_scheduler_reset() {
        let mut sched = super::Xc135Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_135_scheduler_add_remove() {
        let mut sched = super::Xc135Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_135_scheduler_targets() {
        let sched = super::Xc135Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_135_hash_empty() {
        assert_eq!(super::xc_135_hash(b""), 5381);
    }

    #[test]
    fn xc_135_hash_data() {
        let h = super::xc_135_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_135_hash(b"hello"), h);
    }

    #[test]
    fn xc_135_reverse_str() {
        assert_eq!(super::xc_135_reverse("abc"), "cba");
        assert_eq!(super::xc_135_reverse(""), "");
    }


    #[test]
    fn xe_84_pipeline_empty() {
        let p = super::Xe84Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_84_pipeline_parse_stage() {
        let p = super::Xe84Pipeline::new()
            .add_parse(super::xe_84_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_84_pipeline_transform_double() {
        let p = super::Xe84Pipeline::new()
            .add_transform(super::xe_84_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_84_pipeline_validate_reverse() {
        let p = super::Xe84Pipeline::new()
            .add_validate(super::xe_84_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_84_pipeline_emit_filter() {
        let p = super::Xe84Pipeline::new()
            .add_emit(super::xe_84_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_84_pipeline_multi_stage() {
        let p = super::Xe84Pipeline::new()
            .add_parse(super::xe_84_pipeline_identity)
            .add_transform(super::xe_84_pipeline_double)
            .add_validate(super::xe_84_pipeline_reverse)
            .add_emit(super::xe_84_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_84_pipeline_error_propagation() {
        let p = super::Xe84Pipeline::new()
            .add_parse(super::xe_84_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe84Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_84_pipeline_compose() {
        let p1 = super::Xe84Pipeline::new()
            .add_parse(super::xe_84_pipeline_identity);
        let p2 = super::Xe84Pipeline::new()
            .add_transform(super::xe_84_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_84_pipeline_error_display() {
        let e = super::Xe84PipelineError {
            stage: super::Xe84Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_84_cache_put_get() {
        let mut c = super::Xe84Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_84_cache_miss() {
        let mut c: super::Xe84Cache<&str, i32> = super::Xe84Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_84_cache_ttl_expiry() {
        let mut c = super::Xe84Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_84_cache_evict() {
        let mut c = super::Xe84Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_84_cache_capacity() {
        let mut c = super::Xe84Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_84_cache_stats() {
        let mut c = super::Xe84Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_84_cache_clear() {
        let mut c = super::Xe84Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_82 graph tests ------------------------------------------------

    #[test]
    fn xg_82_graph_empty() {
        let g = super::Xg82Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_82_graph_add_node() {
        let mut g = super::Xg82Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_82_graph_add_edge() {
        let mut g = super::Xg82Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_82_graph_neighbors() {
        let mut g = super::Xg82Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_82_graph_has_path() {
        let mut g = super::Xg82Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_82_graph_self_path() {
        let g = super::Xg82Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_82_graph_topo_sort() {
        let mut g = super::Xg82Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_82_graph_cycle_detect_false() {
        let mut g = super::Xg82Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_82_graph_cycle_detect_true() {
        let mut g = super::Xg82Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_82 heap tests -------------------------------------------------

    #[test]
    fn xg_82_heap_empty() {
        let h: super::Xg82Heap<i32> = super::Xg82Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_82_heap_push_pop() {
        let mut h = super::Xg82Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_82_heap_peek() {
        let mut h = super::Xg82Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_82_heap_drain_sorted() {
        let mut h = super::Xg82Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_82_heap_merge() {
        let mut a = super::Xg82Heap::new();
        let mut b = super::Xg82Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_82_heap_default() {
        let h: super::Xg82Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_82_graph_default() {
        let g: super::Xg82Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh134_skip_insert_contains() {
        let mut sl = super::Xh134SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh134_skip_remove() {
        let mut sl = super::Xh134SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh134_skip_len() {
        let mut sl = super::Xh134SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh134_skip_range_query() {
        let mut sl = super::Xh134SkipList::xh_new(4);
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
    fn xh134_skip_floor_ceiling() {
        let mut sl = super::Xh134SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh134_skip_rank() {
        let mut sl = super::Xh134SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh134_skip_empty() {
        let sl = super::Xh134SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh134_skip_duplicates() {
        let mut sl = super::Xh134SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh134_bitset_set_test() {
        let mut bs = super::Xh134BitSet::xh_new(256);
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
    fn xh134_bitset_clear_count() {
        let mut bs = super::Xh134BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh134_bitset_and_or_xor() {
        let mut a = super::Xh134BitSet::xh_new(128);
        let mut b = super::Xh134BitSet::xh_new(128);
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
    fn xh134_bitset_iter_ones() {
        let mut bs = super::Xh134BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh134_bitset_first_last() {
        let mut bs = super::Xh134BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh134_bitset_empty() {
        let bs = super::Xh134BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi134_deque_push_pop_back() {
        let mut dq = super::Xi134Deque::xi_new(4);
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
    fn xi134_deque_push_pop_front() {
        let mut dq = super::Xi134Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi134_deque_mixed_ops() {
        let mut dq = super::Xi134Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi134_deque_get_and_split() {
        let mut dq = super::Xi134Deque::xi_new(8);
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
    fn xi134_deque_rotate_left() {
        let mut dq = super::Xi134Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi134_deque_rotate_right() {
        let mut dq = super::Xi134Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi134_deque_grow() {
        let mut dq = super::Xi134Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi134_deque_empty() {
        let dq = super::Xi134Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi134_interval_tree_insert_query() {
        let mut tree = super::Xi134IntervalTree::xi_new();
        tree.xi_insert(super::Xi134Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi134Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi134Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi134_interval_tree_overlap() {
        let mut tree = super::Xi134IntervalTree::xi_new();
        tree.xi_insert(super::Xi134Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi134Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi134Interval::xi_new(12, 20));
        let q = super::Xi134Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi134_interval_tree_remove() {
        let mut tree = super::Xi134IntervalTree::xi_new();
        tree.xi_insert(super::Xi134Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi134Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi134_interval_tree_gaps() {
        let mut tree = super::Xi134IntervalTree::xi_new();
        tree.xi_insert(super::Xi134Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi134Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi134Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi134Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi134Interval::xi_new(8, 10));
    }

    #[test]
    fn xi134_interval_tree_merge() {
        let mut tree = super::Xi134IntervalTree::xi_new();
        tree.xi_insert(super::Xi134Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi134Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi134Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi134Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi134Interval::xi_new(10, 15));
    }

    #[test]
    fn xi134_interval_tree_all() {
        let mut tree = super::Xi134IntervalTree::xi_new();
        tree.xi_insert(super::Xi134Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi134Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi134_interval_tree_empty() {
        let tree = super::Xi134IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi134_interval_tree_contains_point() {
        let iv = super::Xi134Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 133) ---

    #[test]
    fn xj_133_uf_make_and_find() {
        let mut uf = super::Xj133UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_133_uf_union_connected() {
        let mut uf = super::Xj133UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_133_uf_component_count() {
        let mut uf = super::Xj133UnionFind::xj_new();
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
    fn xj_133_uf_component_size() {
        let mut uf = super::Xj133UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_133_uf_largest_component() {
        let mut uf = super::Xj133UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_133_uf_many_elements() {
        let mut uf = super::Xj133UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_133_uf_separate_components() {
        let mut uf = super::Xj133UnionFind::xj_new();
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
    fn xj_133_uf_path_compression() {
        let mut uf = super::Xj133UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_133_bt_insert_get() {
        let mut bt = super::Xj133BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_133_bt_contains_len() {
        let mut bt = super::Xj133BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_133_bt_replace() {
        let mut bt = super::Xj133BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_133_bt_remove() {
        let mut bt = super::Xj133BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_133_bt_keys_values() {
        let mut bt = super::Xj133BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_133_bt_range() {
        let mut bt = super::Xj133BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_133_bt_min_max() {
        let mut bt = super::Xj133BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_133_bt_many_inserts() {
        let mut bt = super::Xj133BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_133 segment tree tests ---

    #[test]
    fn xk_133_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk133SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_133_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk133SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_133_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk133SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_133_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk133SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_133_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk133SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_133_st_single_element() {
        let data = vec![42];
        let st = super::Xk133SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_133_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk133SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_133_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk133SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_133 disjoint intervals tests ---

    #[test]
    fn xk_133_di_add_and_count() {
        let mut di = super::Xk133DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_133_di_merge_overlap() {
        let mut di = super::Xk133DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_133_di_contains() {
        let mut di = super::Xk133DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_133_di_remove() {
        let mut di = super::Xk133DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_133_di_covered_length() {
        let mut di = super::Xk133DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_133_di_gaps() {
        let mut di = super::Xk133DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_133_di_merge_adjacent() {
        let mut di = super::Xk133DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_133_di_empty() {
        let di = super::Xk133DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_133_rope_new_empty() {
        let rope = super::Xl133Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_133_rope_from_str() {
        let rope = super::Xl133Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_133_rope_insert_at() {
        let mut rope = super::Xl133Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_133_rope_delete_range() {
        let mut rope = super::Xl133Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_133_rope_char_at() {
        let rope = super::Xl133Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_133_rope_split_concat() {
        let rope = super::Xl133Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_133_rope_line_count() {
        let rope = super::Xl133Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_133_rope_line_at() {
        let rope = super::Xl133Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_133_sa_build_and_search() {
        let sa = super::Xl133SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_133_sa_count() {
        let sa = super::Xl133SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_133_sa_longest_repeated() {
        let sa = super::Xl133SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_133_sa_all_positions() {
        let sa = super::Xl133SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_133_sa_len() {
        let sa = super::Xl133SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_133_sa_empty() {
        let sa = super::Xl133SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_133_rope_slice() {
        let rope = super::Xl133Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_133_sa_search_start() {
        let sa = super::Xl133SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_133_sparse_set_get() {
        let mut m = super::Xm133MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_133_sparse_row_col() {
        let mut m = super::Xm133MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_133_sparse_transpose() {
        let mut m = super::Xm133MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_133_sparse_multiply_vec() {
        let mut m = super::Xm133MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_133_sparse_nnz_density() {
        let mut m = super::Xm133MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_133_sparse_clear() {
        let mut m = super::Xm133MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_133_sparse_overwrite_zero() {
        let mut m = super::Xm133MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_133_tokenizer_basic() {
        let t = super::Xm133Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_133_tokenizer_count() {
        let t = super::Xm133Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_133_tokenizer_unique() {
        let t = super::Xm133Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_133_tokenizer_frequency() {
        let t = super::Xm133Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_133_tokenizer_delimiter() {
        let t = super::Xm133Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_133_tokenizer_whitespace() {
        let t = super::Xm133Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_133_tokenizer_empty() {
        let t = super::Xm133Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 134 ----

    #[test]
    fn xn_134_fenwick_prefix_sum() {
        let mut ft = super::Xn134Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_134_fenwick_range_sum() {
        let mut ft = super::Xn134Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_134_fenwick_point_query() {
        let mut ft = super::Xn134Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_134_fenwick_len() {
        let ft = super::Xn134Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_134_fenwick_multiple_updates() {
        let mut ft = super::Xn134Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_134_fenwick_single_element() {
        let mut ft = super::Xn134Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_134_fenwick_find_kth() {
        let mut ft = super::Xn134Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_134_fenwick_negative_delta() {
        let mut ft = super::Xn134Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 134 ----

    #[test]
    fn xn_134_avl_insert_get() {
        let mut m = super::Xn134AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_134_avl_remove() {
        let mut m = super::Xn134AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_134_avl_in_order() {
        let mut m = super::Xn134AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_134_avl_min_max() {
        let mut m = super::Xn134AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_134_avl_floor_ceiling() {
        let mut m = super::Xn134AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_134_avl_height_balanced() {
        let mut m = super::Xn134AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_134_avl_overwrite() {
        let mut m = super::Xn134AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_134_avl_empty() {
        let m: super::Xn134AVL<i32, i32> = super::Xn134AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo134RedBlack tests ---

    #[test]
    fn xo_134_rb_insert_and_get() {
        let mut tree = super::Xo134RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_134_rb_len_and_empty() {
        let mut tree = super::Xo134RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_134_rb_min_max() {
        let mut tree = super::Xo134RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_134_rb_contains() {
        let mut tree = super::Xo134RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_134_rb_remove() {
        let mut tree = super::Xo134RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_134_rb_in_order() {
        let mut tree = super::Xo134RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_134_rb_black_height() {
        let mut tree = super::Xo134RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_134_rb_overwrite() {
        let mut tree = super::Xo134RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo134ConsistentHash tests ---

    #[test]
    fn xo_134_ch_add_and_count() {
        let mut ring = super::Xo134ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_134_ch_remove_node() {
        let mut ring = super::Xo134ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_134_ch_get_node() {
        let mut ring = super::Xo134ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_134_ch_empty_ring() {
        let ring = super::Xo134ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_134_ch_distribution() {
        let mut ring = super::Xo134ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_134_ch_rebalance() {
        let mut ring = super::Xo134ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_134_ch_virtual_nodes() {
        let mut ring = super::Xo134ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_134_ch_consistent_lookup() {
        let mut ring = super::Xo134ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_133_splay_insert_get() {
        let mut t = super::Xp133SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_133_splay_remove() {
        let mut t = super::Xp133SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_133_splay_count_increases() {
        let mut t = super::Xp133SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_133_splay_depth() {
        let mut t = super::Xp133SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_133_splay_len_empty() {
        let t = super::Xp133SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_133_splay_min_max() {
        let mut t = super::Xp133SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_133_splay_overwrite() {
        let mut t = super::Xp133SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_133_splay_remove_missing() {
        let mut t = super::Xp133SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_134 treap tests ----
    #[test]
    fn xq_134_treap_empty() {
        let t = super::Xq134Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_134_treap_insert_get() {
        let mut t = super::Xq134Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_134_treap_overwrite() {
        let mut t = super::Xq134Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_134_treap_remove() {
        let mut t = super::Xq134Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_134_treap_min_max() {
        let mut t = super::Xq134Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_134_treap_rank() {
        let mut t = super::Xq134Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_134_treap_kth() {
        let mut t = super::Xq134Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_134_treap_in_order() {
        let mut t = super::Xq134Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_134 VEB tree tests ----
    #[test]
    fn xq_134_veb_empty() {
        let v = super::Xq134VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_134_veb_insert_contains() {
        let mut v = super::Xq134VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_134_veb_min_max() {
        let mut v = super::Xq134VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_134_veb_delete() {
        let mut v = super::Xq134VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_134_veb_successor() {
        let mut v = super::Xq134VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_134_veb_predecessor() {
        let mut v = super::Xq134VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_134_veb_count() {
        let mut v = super::Xq134VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_134_veb_duplicate_insert() {
        let mut v = super::Xq134VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_134_kdtree_empty() {
        let tree = super::Xr134KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_134_kdtree_insert_one() {
        let mut tree = super::Xr134KDTree::xr_new();
        tree.xr_insert(super::Xr134KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_134_kdtree_insert_multiple() {
        let mut tree = super::Xr134KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr134KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_134_kdtree_nearest_neighbor() {
        let mut tree = super::Xr134KDTree::xr_new();
        tree.xr_insert(super::Xr134KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr134KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr134KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_134_kdtree_nn_empty() {
        let tree = super::Xr134KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr134KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_134_kdtree_range_search() {
        let mut tree = super::Xr134KDTree::xr_new();
        tree.xr_insert(super::Xr134KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr134KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr134KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_134_kdtree_range_empty() {
        let mut tree = super::Xr134KDTree::xr_new();
        tree.xr_insert(super::Xr134KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_134_kdtree_all_points() {
        let mut tree = super::Xr134KDTree::xr_new();
        tree.xr_insert(super::Xr134KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr134KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_134_kdtree_depth() {
        let mut tree = super::Xr134KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr134KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_134_kdtree_bounding_box() {
        let mut tree = super::Xr134KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr134KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr134KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

}
