//! Inter-process communication.

use std::fmt;

/// Errors that can occur during IPC operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcError {
    ChannelNotFound(String),
    ChannelDisconnected(String),
    MessageTooLarge { size: usize, max: usize },
    DuplicateChannel(String),
}

impl fmt::Display for IpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpcError::ChannelNotFound(name) => write!(f, "channel not found: {name}"),
            IpcError::ChannelDisconnected(name) => write!(f, "channel disconnected: {name}"),
            IpcError::MessageTooLarge { size, max } => {
                write!(f, "message too large: {size} bytes (max {max})")
            }
            IpcError::DuplicateChannel(name) => write!(f, "duplicate channel: {name}"),
        }
    }
}

/// A message transmitted over an IPC channel.
#[derive(Debug, Clone)]
pub struct IpcMessage {
    pub id: u64,
    pub channel: String,
    pub payload: Vec<u8>,
}

impl IpcMessage {
    /// Attempt to interpret the payload as a UTF-8 string.
    pub fn payload_as_string(&self) -> Option<String> {
        std::str::from_utf8(&self.payload).ok().map(String::from)
    }
}

impl fmt::Display for IpcMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IpcMessage(id={}, channel={}, {} bytes)",
            self.id,
            self.channel,
            self.payload.len()
        )
    }
}

/// Builder for constructing [`IpcMessage`] instances.
#[derive(Debug, Clone, Default)]
pub struct IpcMessageBuilder {
    id: Option<u64>,
    channel: Option<String>,
    payload: Option<Vec<u8>>,
}

impl IpcMessageBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }

    pub fn channel(mut self, channel: &str) -> Self {
        self.channel = Some(channel.to_string());
        self
    }

    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Consume the builder and produce an `IpcMessage`.
    /// Panics if `id` or `channel` have not been set.
    pub fn build(self) -> IpcMessage {
        IpcMessage {
            id: self.id.expect("IpcMessageBuilder: id is required"),
            channel: self.channel.expect("IpcMessageBuilder: channel is required"),
            payload: self.payload.unwrap_or_default(),
        }
    }
}

/// A named communication channel.
#[derive(Debug, Clone)]
pub struct IpcChannel {
    pub name: String,
    pub connected: bool,
}

/// Wire protocol for IPC communication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcProtocol {
    JsonRpc,
    Binary,
    Custom(String),
}

/// The kind of an IPC message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Request,
    Response,
    Notification,
}

impl fmt::Display for MessageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageKind::Request => write!(f, "Request"),
            MessageKind::Response => write!(f, "Response"),
            MessageKind::Notification => write!(f, "Notification"),
        }
    }
}

/// Header for an IPC message.
#[derive(Debug, Clone)]
pub struct IpcMessageHeader {
    pub kind: MessageKind,
    pub id: u64,
    pub method: Option<String>,
}

/// Service that manages IPC channels and message dispatch.
pub struct IpcService {
    channels: Vec<IpcChannel>,
    next_message_id: u64,
    pending_count: usize,
}

impl IpcService {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            next_message_id: 1,
            pending_count: 0,
        }
    }

    /// Register a new channel by name. Returns `true` if added, `false` if it
    /// already exists.
    pub fn register_channel(&mut self, name: &str) -> bool {
        if self.channels.iter().any(|c| c.name == name) {
            return false;
        }
        self.channels.push(IpcChannel {
            name: name.to_string(),
            connected: true,
        });
        true
    }

    /// Send a message on the named channel. Returns the assigned message id,
    /// or `None` if the channel does not exist or is disconnected.
    pub fn send_message(&mut self, channel: &str, payload: Vec<u8>) -> Option<u64> {
        let ch = self.channels.iter().find(|c| c.name == channel)?;
        if !ch.connected {
            return None;
        }
        let id = self.next_message_id;
        self.next_message_id += 1;
        self.pending_count += 1;
        // In a real implementation the message would be serialised and sent.
        let _ = IpcMessage {
            id,
            channel: channel.to_string(),
            payload,
        };
        Some(id)
    }

    /// Build a request message for a given method.
    pub fn create_request(&mut self, method: &str, payload: Vec<u8>) -> IpcMessage {
        let id = self.next_message_id;
        self.next_message_id += 1;
        IpcMessage {
            id,
            channel: method.to_string(),
            payload,
        }
    }

    /// Build a response message for a previously received request.
    pub fn create_response(&mut self, request_id: u64, payload: Vec<u8>) -> IpcMessage {
        IpcMessage {
            id: request_id,
            channel: String::new(),
            payload,
        }
    }

    /// Mark a channel as disconnected. Returns an error if the channel is not
    /// found.
    pub fn disconnect_channel(&mut self, name: &str) -> Result<(), IpcError> {
        let ch = self
            .channels
            .iter_mut()
            .find(|c| c.name == name)
            .ok_or_else(|| IpcError::ChannelNotFound(name.to_string()))?;
        ch.connected = false;
        Ok(())
    }

    /// Re-connect a previously disconnected channel. Returns an error if the
    /// channel is not found.
    pub fn reconnect_channel(&mut self, name: &str) -> Result<(), IpcError> {
        let ch = self
            .channels
            .iter_mut()
            .find(|c| c.name == name)
            .ok_or_else(|| IpcError::ChannelNotFound(name.to_string()))?;
        ch.connected = true;
        Ok(())
    }

    /// Remove a channel entirely. Returns an error if the channel is not found.
    pub fn unregister_channel(&mut self, name: &str) -> Result<(), IpcError> {
        let idx = self
            .channels
            .iter()
            .position(|c| c.name == name)
            .ok_or_else(|| IpcError::ChannelNotFound(name.to_string()))?;
        self.channels.remove(idx);
        Ok(())
    }

    /// Return a list of `(channel_name, connected)` pairs.
    pub fn list_channels(&self) -> Vec<(String, bool)> {
        self.channels
            .iter()
            .map(|c| (c.name.clone(), c.connected))
            .collect()
    }

    /// Create a notification message (fire-and-forget, no response expected).
    pub fn send_notification(&mut self, channel: &str, payload: Vec<u8>) -> Result<IpcMessage, IpcError> {
        let ch = self
            .channels
            .iter()
            .find(|c| c.name == channel)
            .ok_or_else(|| IpcError::ChannelNotFound(channel.to_string()))?;
        if !ch.connected {
            return Err(IpcError::ChannelDisconnected(channel.to_string()));
        }
        let id = self.next_message_id;
        self.next_message_id += 1;
        self.pending_count += 1;
        Ok(IpcMessage {
            id,
            channel: channel.to_string(),
            payload,
        })
    }

    /// Number of messages sent through the service so far.
    pub fn pending_count(&self) -> usize {
        self.pending_count
    }
}

impl Default for IpcService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_send() {
        let mut svc = IpcService::new();
        assert!(svc.register_channel("editor"));
        assert!(!svc.register_channel("editor")); // duplicate

        let id = svc.send_message("editor", b"hello".to_vec());
        assert!(id.is_some());

        assert!(svc.send_message("nonexistent", vec![]).is_none());
    }

    #[test]
    fn create_request_and_response() {
        let mut svc = IpcService::new();
        let req = svc.create_request("textDocument/completion", b"{}".to_vec());
        assert_eq!(req.channel, "textDocument/completion");

        let resp = svc.create_response(req.id, b"result".to_vec());
        assert_eq!(resp.id, req.id);
    }

    #[test]
    fn message_header_kinds() {
        let header = IpcMessageHeader {
            kind: MessageKind::Notification,
            id: 0,
            method: Some("initialized".into()),
        };
        assert_eq!(header.kind, MessageKind::Notification);
        assert_eq!(header.method.as_deref(), Some("initialized"));

        assert_ne!(IpcProtocol::JsonRpc, IpcProtocol::Binary);
        assert_eq!(
            IpcProtocol::Custom("grpc".into()),
            IpcProtocol::Custom("grpc".into())
        );
    }

    #[test]
    fn disconnect_and_reconnect_channel() {
        let mut svc = IpcService::new();
        svc.register_channel("lsp");
        assert!(svc.send_message("lsp", b"ping".to_vec()).is_some());

        svc.disconnect_channel("lsp").unwrap();
        assert!(svc.send_message("lsp", b"ping".to_vec()).is_none());

        svc.reconnect_channel("lsp").unwrap();
        assert!(svc.send_message("lsp", b"ping".to_vec()).is_some());
    }

    #[test]
    fn disconnect_unknown_channel_returns_error() {
        let mut svc = IpcService::new();
        let err = svc.disconnect_channel("ghost").unwrap_err();
        assert_eq!(err, IpcError::ChannelNotFound("ghost".into()));
    }

    #[test]
    fn unregister_channel() {
        let mut svc = IpcService::new();
        svc.register_channel("tmp");
        assert_eq!(svc.list_channels().len(), 1);
        svc.unregister_channel("tmp").unwrap();
        assert!(svc.list_channels().is_empty());
        assert_eq!(
            svc.unregister_channel("tmp").unwrap_err(),
            IpcError::ChannelNotFound("tmp".into())
        );
    }

    #[test]
    fn list_channels_shows_status() {
        let mut svc = IpcService::new();
        svc.register_channel("a");
        svc.register_channel("b");
        svc.disconnect_channel("b").unwrap();

        let list = svc.list_channels();
        assert_eq!(list, vec![("a".into(), true), ("b".into(), false)]);
    }

    #[test]
    fn send_notification_success_and_disconnected() {
        let mut svc = IpcService::new();
        svc.register_channel("events");

        let msg = svc.send_notification("events", b"data".to_vec()).unwrap();
        assert_eq!(msg.channel, "events");
        assert_eq!(msg.payload, b"data");

        svc.disconnect_channel("events").unwrap();
        let err = svc.send_notification("events", vec![]).unwrap_err();
        assert_eq!(err, IpcError::ChannelDisconnected("events".into()));
    }

    #[test]
    fn send_notification_unknown_channel() {
        let mut svc = IpcService::new();
        let err = svc.send_notification("nope", vec![]).unwrap_err();
        assert_eq!(err, IpcError::ChannelNotFound("nope".into()));
    }

    #[test]
    fn payload_as_string_valid_and_invalid() {
        let valid = IpcMessage {
            id: 1,
            channel: "ch".into(),
            payload: b"hello world".to_vec(),
        };
        assert_eq!(valid.payload_as_string(), Some("hello world".into()));

        let invalid = IpcMessage {
            id: 2,
            channel: "ch".into(),
            payload: vec![0xff, 0xfe],
        };
        assert!(invalid.payload_as_string().is_none());
    }

    #[test]
    fn ipc_message_display() {
        let msg = IpcMessage {
            id: 42,
            channel: "editor".into(),
            payload: vec![0; 10],
        };
        assert_eq!(format!("{msg}"), "IpcMessage(id=42, channel=editor, 10 bytes)");
    }

    #[test]
    fn message_kind_display() {
        assert_eq!(format!("{}", MessageKind::Request), "Request");
        assert_eq!(format!("{}", MessageKind::Response), "Response");
        assert_eq!(format!("{}", MessageKind::Notification), "Notification");
    }

    #[test]
    fn ipc_message_builder() {
        let msg = IpcMessageBuilder::new()
            .id(7)
            .channel("build")
            .payload(b"content".to_vec())
            .build();
        assert_eq!(msg.id, 7);
        assert_eq!(msg.channel, "build");
        assert_eq!(msg.payload, b"content");
    }

    #[test]
    fn ipc_message_builder_default_payload() {
        let msg = IpcMessageBuilder::new().id(1).channel("c").build();
        assert!(msg.payload.is_empty());
    }

    #[test]
    fn error_display() {
        assert_eq!(
            IpcError::ChannelNotFound("x".into()).to_string(),
            "channel not found: x"
        );
        assert_eq!(
            IpcError::ChannelDisconnected("y".into()).to_string(),
            "channel disconnected: y"
        );
        assert_eq!(
            IpcError::MessageTooLarge { size: 100, max: 50 }.to_string(),
            "message too large: 100 bytes (max 50)"
        );
        assert_eq!(
            IpcError::DuplicateChannel("z".into()).to_string(),
            "duplicate channel: z"
        );
    }

    #[test]
    fn pending_count_tracks_messages() {
        let mut svc = IpcService::new();
        svc.register_channel("ch");
        assert_eq!(svc.pending_count(), 0);

        svc.send_message("ch", b"a".to_vec());
        svc.send_message("ch", b"b".to_vec());
        assert_eq!(svc.pending_count(), 2);

        svc.send_notification("ch", b"c".to_vec()).unwrap();
        assert_eq!(svc.pending_count(), 3);
    }
}
