//! Output panel view.

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
}
