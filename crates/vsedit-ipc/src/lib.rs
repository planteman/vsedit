//! Inter-process communication.

/// A message transmitted over an IPC channel.
#[derive(Debug, Clone)]
pub struct IpcMessage {
    pub id: u64,
    pub channel: String,
    pub payload: Vec<u8>,
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
}

impl IpcService {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            next_message_id: 1,
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
}
