//! Output panel channels.

use std::fmt;

/// Errors returned by output channel operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputError {
    /// The requested channel id was not found.
    ChannelNotFound(String),
    /// A channel with the given id already exists.
    DuplicateChannel(String),
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::ChannelNotFound(id) => write!(f, "channel not found: {id}"),
            OutputError::DuplicateChannel(id) => write!(f, "duplicate channel: {id}"),
        }
    }
}

/// Descriptor for an output channel.
#[derive(Debug, Clone)]
pub struct OutputChannelDescriptor {
    pub id: String,
    pub name: String,
    pub language_id: Option<String>,
    pub log: bool,
}

impl OutputChannelDescriptor {
    /// Builder method to set the language id.
    pub fn with_language(mut self, language_id: &str) -> Self {
        self.language_id = Some(language_id.to_string());
        self
    }
}

impl fmt::Display for OutputChannelDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, self.id)?;
        if let Some(lang) = &self.language_id {
            write!(f, " [{lang}]")?;
        }
        Ok(())
    }
}

/// Internal state for an output channel.
#[derive(Debug, Clone)]
pub struct OutputChannelState {
    pub descriptor: OutputChannelDescriptor,
    pub content: String,
    pub visible: bool,
}

impl OutputChannelState {
    /// Returns the number of lines in the content.
    pub fn line_count(&self) -> usize {
        if self.content.is_empty() {
            0
        } else {
            self.content.lines().count()
        }
    }
}

impl fmt::Display for OutputChannelState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vis = if self.visible { "visible" } else { "hidden" };
        write!(f, "{} ({}, {} lines)", self.descriptor, vis, self.line_count())
    }
}

/// Service for managing output channels.
pub struct OutputChannelService {
    channels: Vec<OutputChannelState>,
    active: Option<String>,
}

impl OutputChannelService {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            active: None,
        }
    }

    pub fn create_channel(&mut self, descriptor: OutputChannelDescriptor) -> String {
        let id = descriptor.id.clone();
        self.channels.push(OutputChannelState {
            descriptor,
            content: String::new(),
            visible: false,
        });
        id
    }

    pub fn append(&mut self, id: &str, text: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.descriptor.id == id) {
            ch.content.push_str(text);
        }
    }

    pub fn append_line(&mut self, id: &str, text: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.descriptor.id == id) {
            ch.content.push_str(text);
            ch.content.push('\n');
        }
    }

    pub fn clear(&mut self, id: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.descriptor.id == id) {
            ch.content.clear();
        }
    }

    pub fn get_content(&self, id: &str) -> Option<&str> {
        self.channels
            .iter()
            .find(|c| c.descriptor.id == id)
            .map(|c| c.content.as_str())
    }

    pub fn show(&mut self, id: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.descriptor.id == id) {
            ch.visible = true;
        }
    }

    pub fn hide(&mut self, id: &str) {
        if let Some(ch) = self.channels.iter_mut().find(|c| c.descriptor.id == id) {
            ch.visible = false;
        }
    }

    pub fn set_active(&mut self, id: &str) {
        if self.channels.iter().any(|c| c.descriptor.id == id) {
            self.active = Some(id.to_string());
        }
    }

    pub fn get_active(&self) -> Option<&OutputChannelState> {
        self.active
            .as_ref()
            .and_then(|id| self.channels.iter().find(|c| c.descriptor.id == *id))
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Remove a channel by id. Returns an error if the channel does not exist.
    pub fn remove_channel(&mut self, id: &str) -> Result<(), OutputError> {
        let pos = self
            .channels
            .iter()
            .position(|c| c.descriptor.id == id)
            .ok_or_else(|| OutputError::ChannelNotFound(id.to_string()))?;
        self.channels.remove(pos);
        if self.active.as_deref() == Some(id) {
            self.active = None;
        }
        Ok(())
    }

    /// Get a reference to a channel's state by id.
    pub fn get_channel(&self, id: &str) -> Option<&OutputChannelState> {
        self.channels.iter().find(|c| c.descriptor.id == id)
    }

    /// Find the first channel whose name matches the given string.
    pub fn find_by_name(&self, name: &str) -> Option<&OutputChannelState> {
        self.channels.iter().find(|c| c.descriptor.name == name)
    }

    /// Replace all content in a channel. Returns an error if the channel does not exist.
    pub fn replace_content(&mut self, id: &str, content: &str) -> Result<(), OutputError> {
        let ch = self
            .channels
            .iter_mut()
            .find(|c| c.descriptor.id == id)
            .ok_or_else(|| OutputError::ChannelNotFound(id.to_string()))?;
        ch.content = content.to_string();
        Ok(())
    }

    /// Get the number of lines in a channel's content.
    pub fn get_line_count(&self, id: &str) -> Result<usize, OutputError> {
        let ch = self
            .channels
            .iter()
            .find(|c| c.descriptor.id == id)
            .ok_or_else(|| OutputError::ChannelNotFound(id.to_string()))?;
        Ok(ch.line_count())
    }

    /// Search all channels for content containing the given query string.
    /// Returns a list of (channel id, matching line) pairs.
    pub fn search_content(&self, query: &str) -> Vec<(&str, &str)> {
        let mut results = Vec::new();
        for ch in &self.channels {
            for line in ch.content.lines() {
                if line.contains(query) {
                    results.push((ch.descriptor.id.as_str(), line));
                }
            }
        }
        results
    }
}

impl Default for OutputChannelService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(id: &str) -> OutputChannelDescriptor {
        OutputChannelDescriptor {
            id: id.to_string(),
            name: id.to_string(),
            language_id: None,
            log: false,
        }
    }

    fn named_desc(id: &str, name: &str) -> OutputChannelDescriptor {
        OutputChannelDescriptor {
            id: id.to_string(),
            name: name.to_string(),
            language_id: None,
            log: false,
        }
    }

    #[test]
    fn create_and_append() {
        let mut svc = OutputChannelService::new();
        let id = svc.create_channel(desc("out"));
        svc.append(&id, "hello ");
        svc.append_line(&id, "world");
        assert_eq!(svc.get_content(&id), Some("hello world\n"));
    }

    #[test]
    fn clear_content() {
        let mut svc = OutputChannelService::new();
        let id = svc.create_channel(desc("out"));
        svc.append(&id, "data");
        svc.clear(&id);
        assert_eq!(svc.get_content(&id), Some(""));
    }

    #[test]
    fn active_channel() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("a"));
        svc.create_channel(desc("b"));
        assert!(svc.get_active().is_none());
        svc.set_active("b");
        assert_eq!(svc.get_active().unwrap().descriptor.id, "b");
        assert_eq!(svc.channel_count(), 2);
    }

    #[test]
    fn output_error_display() {
        let e1 = OutputError::ChannelNotFound("x".into());
        assert_eq!(e1.to_string(), "channel not found: x");
        let e2 = OutputError::DuplicateChannel("y".into());
        assert_eq!(e2.to_string(), "duplicate channel: y");
    }

    #[test]
    fn descriptor_display_without_language() {
        let d = desc("log");
        assert_eq!(d.to_string(), "log(log)");
    }

    #[test]
    fn descriptor_display_with_language() {
        let d = desc("log").with_language("rust");
        assert_eq!(d.to_string(), "log(log) [rust]");
    }

    #[test]
    fn with_language_builder() {
        let d = desc("ch").with_language("json");
        assert_eq!(d.language_id.as_deref(), Some("json"));
    }

    #[test]
    fn state_display_and_line_count() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("s"));
        svc.append_line("s", "line1");
        svc.append_line("s", "line2");
        let ch = svc.get_channel("s").unwrap();
        assert_eq!(ch.line_count(), 2);
        assert_eq!(ch.to_string(), "s(s) (hidden, 2 lines)");
    }

    #[test]
    fn remove_channel_success() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("r"));
        assert_eq!(svc.channel_count(), 1);
        svc.remove_channel("r").unwrap();
        assert_eq!(svc.channel_count(), 0);
    }

    #[test]
    fn remove_channel_clears_active() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("a"));
        svc.set_active("a");
        svc.remove_channel("a").unwrap();
        assert!(svc.get_active().is_none());
    }

    #[test]
    fn remove_channel_not_found() {
        let mut svc = OutputChannelService::new();
        let err = svc.remove_channel("missing").unwrap_err();
        assert_eq!(err, OutputError::ChannelNotFound("missing".into()));
    }

    #[test]
    fn get_channel_and_find_by_name() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(named_desc("id1", "Build Output"));
        assert!(svc.get_channel("id1").is_some());
        assert!(svc.get_channel("nope").is_none());
        assert_eq!(
            svc.find_by_name("Build Output").unwrap().descriptor.id,
            "id1"
        );
        assert!(svc.find_by_name("Other").is_none());
    }

    #[test]
    fn replace_content_success_and_error() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("rc"));
        svc.append("rc", "old");
        svc.replace_content("rc", "new").unwrap();
        assert_eq!(svc.get_content("rc"), Some("new"));
        let err = svc.replace_content("bad", "x").unwrap_err();
        assert_eq!(err, OutputError::ChannelNotFound("bad".into()));
    }

    #[test]
    fn get_line_count_and_empty() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("lc"));
        assert_eq!(svc.get_line_count("lc").unwrap(), 0);
        svc.append_line("lc", "a");
        svc.append_line("lc", "b");
        svc.append_line("lc", "c");
        assert_eq!(svc.get_line_count("lc").unwrap(), 3);
        let err = svc.get_line_count("no").unwrap_err();
        assert_eq!(err, OutputError::ChannelNotFound("no".into()));
    }

    #[test]
    fn search_content_across_channels() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("a"));
        svc.create_channel(desc("b"));
        svc.append_line("a", "error: something failed");
        svc.append_line("a", "info: all good");
        svc.append_line("b", "error: another failure");
        svc.append_line("b", "debug: trace");
        let results = svc.search_content("error");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "a");
        assert_eq!(results[1].0, "b");
    }

    #[test]
    fn search_content_no_matches() {
        let mut svc = OutputChannelService::new();
        svc.create_channel(desc("x"));
        svc.append_line("x", "hello world");
        let results = svc.search_content("zzz");
        assert!(results.is_empty());
    }
}
