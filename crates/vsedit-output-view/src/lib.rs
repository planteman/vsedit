//! Output panel view.

use std::fmt;

/// Errors that can occur when working with output channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputError {
    ChannelNotFound(String),
    DuplicateChannel(String),
    ChannelEmpty,
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::ChannelNotFound(id) => write!(f, "channel not found: {id}"),
            OutputError::DuplicateChannel(id) => write!(f, "duplicate channel: {id}"),
            OutputError::ChannelEmpty => write!(f, "channel is empty"),
        }
    }
}

/// A single output channel that accumulates text lines.
pub struct OutputChannel {
    pub id: String,
    pub name: String,
    pub lines: Vec<String>,
    pub visible: bool,
    pub language_id: Option<String>,
}

impl OutputChannel {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            lines: Vec::new(),
            visible: false,
            language_id: None,
        }
    }

    pub fn append(&mut self, text: &str) {
        if let Some(last) = self.lines.last_mut() {
            last.push_str(text);
        } else {
            self.lines.push(text.to_string());
        }
    }

    pub fn append_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn get_content(&self) -> String {
        self.lines.join("\n")
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get a specific line by zero-based index.
    pub fn get_line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(|s| s.as_str())
    }

    /// Find all lines containing `pattern`, returning (line_index, line_text).
    pub fn search(&self, pattern: &str) -> Vec<(usize, &str)> {
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(pattern))
            .map(|(i, line)| (i, line.as_str()))
            .collect()
    }

    /// Replace the entire content with new lines.
    pub fn replace(&mut self, lines: Vec<String>) {
        self.lines = lines;
    }

    /// Builder method to set the language_id.
    pub fn with_language(mut self, language_id: impl Into<String>) -> Self {
        self.language_id = Some(language_id.into());
        self
    }

    /// Return the last `n` lines.
    pub fn tail(&self, n: usize) -> Vec<&str> {
        let start = self.lines.len().saturating_sub(n);
        self.lines[start..].iter().map(|s| s.as_str()).collect()
    }
}

impl fmt::Display for OutputChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} lines)", self.name, self.lines.len())
    }
}

/// Service managing multiple output channels.
pub struct OutputService {
    pub channels: Vec<OutputChannel>,
    pub active_channel: Option<usize>,
}

impl OutputService {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            active_channel: None,
        }
    }

    /// Create a new channel and return its index.
    pub fn create_channel(&mut self, name: impl Into<String>) -> usize {
        let idx = self.channels.len();
        let id = format!("channel-{idx}");
        self.channels.push(OutputChannel::new(id, name));
        idx
    }

    pub fn get_channel(&self, id: &str) -> Option<&OutputChannel> {
        self.channels.iter().find(|c| c.id == id)
    }

    pub fn get_channel_mut(&mut self, id: &str) -> Option<&mut OutputChannel> {
        self.channels.iter_mut().find(|c| c.id == id)
    }

    pub fn set_active(&mut self, id: &str) {
        if let Some(idx) = self.channels.iter().position(|c| c.id == id) {
            self.active_channel = Some(idx);
        }
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Remove a channel by id, returning it if found.
    pub fn remove_channel(&mut self, id: &str) -> Result<OutputChannel, OutputError> {
        let idx = self
            .channels
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| OutputError::ChannelNotFound(id.to_string()))?;
        // Adjust active_channel index after removal.
        if let Some(active) = self.active_channel {
            if active == idx {
                self.active_channel = None;
            } else if active > idx {
                self.active_channel = Some(active - 1);
            }
        }
        Ok(self.channels.remove(idx))
    }

    /// Get a reference to the currently active channel.
    pub fn get_active_channel(&self) -> Option<&OutputChannel> {
        self.active_channel.and_then(|i| self.channels.get(i))
    }

    /// Find the first channel whose name matches.
    pub fn find_by_name(&self, name: &str) -> Option<&OutputChannel> {
        self.channels.iter().find(|c| c.name == name)
    }

    /// Clear the contents of every channel.
    pub fn clear_all(&mut self) {
        for ch in &mut self.channels {
            ch.clear();
        }
    }
}

impl Default for OutputService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_append_and_content() {
        let mut ch = OutputChannel::new("ch1", "Build");
        ch.append_line("line 1");
        ch.append_line("line 2");
        assert_eq!(ch.line_count(), 2);
        assert_eq!(ch.get_content(), "line 1\nline 2");
    }

    #[test]
    fn channel_clear() {
        let mut ch = OutputChannel::new("ch1", "Build");
        ch.append_line("hello");
        ch.clear();
        assert_eq!(ch.line_count(), 0);
        assert_eq!(ch.get_content(), "");
    }

    #[test]
    fn channel_visibility() {
        let mut ch = OutputChannel::new("ch1", "Build");
        assert!(!ch.visible);
        ch.show();
        assert!(ch.visible);
        ch.hide();
        assert!(!ch.visible);
    }

    #[test]
    fn service_create_and_find() {
        let mut svc = OutputService::new();
        let idx = svc.create_channel("Build");
        assert_eq!(idx, 0);
        assert_eq!(svc.channel_count(), 1);
        assert!(svc.get_channel("channel-0").is_some());
        assert!(svc.get_channel("nonexistent").is_none());
    }

    #[test]
    fn service_set_active() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        svc.set_active("channel-1");
        assert_eq!(svc.active_channel, Some(1));
    }

    #[test]
    fn output_error_display() {
        assert_eq!(
            OutputError::ChannelNotFound("x".into()).to_string(),
            "channel not found: x"
        );
        assert_eq!(
            OutputError::DuplicateChannel("x".into()).to_string(),
            "duplicate channel: x"
        );
        assert_eq!(OutputError::ChannelEmpty.to_string(), "channel is empty");
    }

    #[test]
    fn channel_display() {
        let mut ch = OutputChannel::new("ch1", "Build");
        assert_eq!(ch.to_string(), "Build (0 lines)");
        ch.append_line("hello");
        ch.append_line("world");
        assert_eq!(ch.to_string(), "Build (2 lines)");
    }

    #[test]
    fn channel_get_line() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("alpha");
        ch.append_line("beta");
        assert_eq!(ch.get_line(0), Some("alpha"));
        assert_eq!(ch.get_line(1), Some("beta"));
        assert_eq!(ch.get_line(2), None);
    }

    #[test]
    fn channel_search() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("error: something failed");
        ch.append_line("info: all good");
        ch.append_line("error: another failure");
        let results = ch.search("error");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], (0, "error: something failed"));
        assert_eq!(results[1], (2, "error: another failure"));
        assert!(ch.search("warning").is_empty());
    }

    #[test]
    fn channel_replace() {
        let mut ch = OutputChannel::new("ch1", "Log");
        ch.append_line("old line");
        ch.replace(vec!["new line 1".into(), "new line 2".into()]);
        assert_eq!(ch.line_count(), 2);
        assert_eq!(ch.get_line(0), Some("new line 1"));
    }

    #[test]
    fn channel_with_language() {
        let ch = OutputChannel::new("ch1", "Build").with_language("rust");
        assert_eq!(ch.language_id.as_deref(), Some("rust"));
    }

    #[test]
    fn channel_tail() {
        let mut ch = OutputChannel::new("ch1", "Log");
        for i in 0..10 {
            ch.append_line(&format!("line {i}"));
        }
        let last3 = ch.tail(3);
        assert_eq!(last3, vec!["line 7", "line 8", "line 9"]);
        assert_eq!(ch.tail(20).len(), 10);
        assert!(OutputChannel::new("ch2", "Empty").tail(5).is_empty());
    }

    #[test]
    fn service_remove_channel() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        svc.set_active("channel-1");
        let removed = svc.remove_channel("channel-0").unwrap();
        assert_eq!(removed.name, "Build");
        assert_eq!(svc.channel_count(), 1);
        // active index should shift down
        assert_eq!(svc.active_channel, Some(0));
        assert!(svc.remove_channel("nonexistent").is_err());
    }

    #[test]
    fn service_get_active_channel() {
        let mut svc = OutputService::new();
        assert!(svc.get_active_channel().is_none());
        svc.create_channel("Build");
        svc.set_active("channel-0");
        assert_eq!(svc.get_active_channel().unwrap().name, "Build");
    }

    #[test]
    fn service_find_by_name() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        assert_eq!(svc.find_by_name("Tests").unwrap().id, "channel-1");
        assert!(svc.find_by_name("Nonexistent").is_none());
    }

    #[test]
    fn service_clear_all() {
        let mut svc = OutputService::new();
        svc.create_channel("Build");
        svc.create_channel("Tests");
        svc.get_channel_mut("channel-0").unwrap().append_line("hello");
        svc.get_channel_mut("channel-1").unwrap().append_line("world");
        svc.clear_all();
        assert_eq!(svc.get_channel("channel-0").unwrap().line_count(), 0);
        assert_eq!(svc.get_channel("channel-1").unwrap().line_count(), 0);
    }
}
