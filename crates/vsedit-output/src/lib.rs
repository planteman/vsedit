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
    fn toggle_auto_scroll() {
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
    fn total_line_count() {
        let mut p = OutputPanel::new();
        p.create_channel("A");
        p.create_channel("B");
        p.append_line(0, "l1");
        p.append_line(0, "l2");
        p.append_line(1, "l3");
        assert_eq!(p.total_line_count(), 3);
    }

    #[test]
    fn channel_names() {
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
    fn remove_channel() {
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
    fn visible_channel_indices() {
        let mut p = OutputPanel::new();
        p.create_channel("A");
        p.create_channel("B");
        p.channels[1].is_visible = false;
        assert_eq!(p.visible_channel_indices(), vec![0]);
        assert_eq!(p.hidden_channel_indices(), vec![1]);
    }

    #[test]
    fn set_channel_visibility() {
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
}
