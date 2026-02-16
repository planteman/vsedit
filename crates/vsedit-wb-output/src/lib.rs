//! Output panel channels.

/// Descriptor for an output channel.
#[derive(Debug, Clone)]
pub struct OutputChannelDescriptor {
    pub id: String,
    pub name: String,
    pub language_id: Option<String>,
    pub log: bool,
}

/// Internal state for an output channel.
#[derive(Debug, Clone)]
pub struct OutputChannelState {
    pub descriptor: OutputChannelDescriptor,
    pub content: String,
    pub visible: bool,
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
}
