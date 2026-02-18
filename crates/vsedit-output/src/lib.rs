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

}
