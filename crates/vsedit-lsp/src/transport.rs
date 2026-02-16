//! Content-Length framed JSON-RPC transport for LSP.

use std::io;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A JSON-RPC request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// A JSON-RPC notification message (no id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// A JSON-RPC response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

/// An inbound message from the language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcMessage {
    /// Returns true if this is a response (has an id but no method).
    pub fn is_response(&self) -> bool {
        self.id.is_some() && self.method.is_none()
    }

    /// Returns true if this is a request (has both id and method).
    pub fn is_request(&self) -> bool {
        self.id.is_some() && self.method.is_some()
    }

    /// Returns true if this is a notification (has method but no id).
    pub fn is_notification(&self) -> bool {
        self.id.is_none() && self.method.is_some()
    }
}

/// Encode a JSON value into a Content-Length framed message.
pub fn encode_message(value: &impl Serialize) -> Vec<u8> {
    let body = serde_json::to_string(value).expect("serialize JSON-RPC message");
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut out = Vec::with_capacity(header.len() + body.len());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(body.as_bytes());
    out
}

/// Try to extract a complete Content-Length framed message from a buffer.
///
/// Returns `Ok(Some((message, consumed)))` on success, `Ok(None)` if not
/// enough data is available, or `Err` on parse failure.
pub fn try_decode_message(buf: &[u8]) -> io::Result<Option<(JsonRpcMessage, usize)>> {
    let header_end = match find_header_end(buf) {
        Some(pos) => pos,
        None => return Ok(None),
    };

    let header = std::str::from_utf8(&buf[..header_end])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let content_length = parse_content_length(header).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
    })?;

    let body_start = header_end + 4; // skip \r\n\r\n
    let total = body_start + content_length;

    if buf.len() < total {
        return Ok(None);
    }

    let body = &buf[body_start..total];
    let msg: JsonRpcMessage = serde_json::from_slice(body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(Some((msg, total)))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
}

fn parse_content_length(header: &str) -> Option<usize> {
    for line in header.split("\r\n") {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("Content-Length:") {
            return val.trim().parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_and_decode_roundtrip() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "initialize".to_string(),
            params: Some(serde_json::json!({})),
        };
        let encoded = encode_message(&req);
        let (msg, consumed) = try_decode_message(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(msg.id, Some(1));
        assert_eq!(msg.method.as_deref(), Some("initialize"));
    }

    #[test]
    fn decode_incomplete_returns_none() {
        let result = try_decode_message(b"Content-Length: 100\r\n\r\n{").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn decode_no_header_returns_none() {
        let result = try_decode_message(b"partial").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn decode_invalid_json_returns_error() {
        let raw = b"Content-Length: 3\r\n\r\nfoo";
        let result = try_decode_message(raw);
        assert!(result.is_err());
    }

    #[test]
    fn notification_encode() {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "initialized".to_string(),
            params: None,
        };
        let encoded = encode_message(&notif);
        let s = String::from_utf8_lossy(&encoded);
        assert!(s.contains("Content-Length:"));
        assert!(s.contains("initialized"));
    }

    #[test]
    fn message_classification() {
        let response = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: None,
            params: None,
            result: Some(serde_json::json!({})),
            error: None,
        };
        assert!(response.is_response());
        assert!(!response.is_request());
        assert!(!response.is_notification());

        let request = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(2),
            method: Some("textDocument/hover".into()),
            params: Some(serde_json::json!({})),
            result: None,
            error: None,
        };
        assert!(request.is_request());
        assert!(!request.is_response());
        assert!(!request.is_notification());

        let notification = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some("textDocument/publishDiagnostics".into()),
            params: Some(serde_json::json!({})),
            result: None,
            error: None,
        };
        assert!(notification.is_notification());
        assert!(!notification.is_request());
        assert!(!notification.is_response());
    }

    #[test]
    fn parse_content_length_various() {
        assert_eq!(parse_content_length("Content-Length: 42"), Some(42));
        assert_eq!(parse_content_length("Content-Length:42"), Some(42));
        assert_eq!(parse_content_length("Content-Type: json\r\nContent-Length: 10"), Some(10));
        assert_eq!(parse_content_length("No-Header: here"), None);
    }

    #[test]
    fn json_rpc_error_serialization() {
        let err = JsonRpcError {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("-32600"));
        let back: JsonRpcError = serde_json::from_str(&json).unwrap();
        assert_eq!(back.code, -32600);
    }
}
