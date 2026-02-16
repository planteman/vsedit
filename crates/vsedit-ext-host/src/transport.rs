//! Line-delimited JSON-RPC transport using the `Content-Length` header protocol.

use std::io::{self, BufRead, BufReader, Read, Write};

use vsedit_ext_rpc::{RpcMessage, RpcProtocol};

/// Encode an [`RpcMessage`] into the `Content-Length` wire format.
pub fn encode_message(msg: &RpcMessage) -> Vec<u8> {
    let body = RpcProtocol::serialize_message(msg);
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut buf = Vec::with_capacity(header.len() + body.len());
    buf.extend_from_slice(header.as_bytes());
    buf.extend_from_slice(body.as_bytes());
    buf
}

/// Read one `Content-Length`-framed message from a buffered reader.
///
/// Returns `Ok(None)` on EOF.
pub fn decode_message<R: Read>(reader: &mut BufReader<R>) -> io::Result<Option<RpcMessage>> {
    // Read headers until the blank line separator.
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None); // EOF
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            );
        }
    }

    let len = content_length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length"))?;

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;

    let text = std::str::from_utf8(&body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    RpcProtocol::deserialize_message(text)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// A synchronous transport that wraps a writer and a buffered reader.
pub struct RpcTransport<W: Write, R: Read> {
    writer: W,
    reader: BufReader<R>,
}

impl<W: Write, R: Read> RpcTransport<W, R> {
    pub fn new(writer: W, reader: R) -> Self {
        Self {
            writer,
            reader: BufReader::new(reader),
        }
    }

    /// Send a message (blocking).
    pub fn send(&mut self, msg: &RpcMessage) -> io::Result<()> {
        let encoded = encode_message(msg);
        self.writer.write_all(&encoded)?;
        self.writer.flush()
    }

    /// Receive a message (blocking). Returns `Ok(None)` on EOF.
    pub fn recv(&mut self) -> io::Result<Option<RpcMessage>> {
        decode_message(&mut self.reader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use vsedit_ext_rpc::{RpcEvent, RpcRequest, RpcResponse};

    fn sample_request() -> RpcMessage {
        RpcMessage::Request(RpcRequest {
            id: 1,
            proxy_id: "MainThreadCommands".into(),
            method: "executeCommand".into(),
            args: vec![json!("workbench.action.files.save")],
        })
    }

    fn sample_response() -> RpcMessage {
        RpcMessage::Response(RpcResponse {
            id: 42,
            result: Ok(json!({"key": "value"})),
        })
    }

    fn sample_event() -> RpcMessage {
        RpcMessage::Event(RpcEvent {
            proxy_id: "ExtHostEditors".into(),
            event_name: "onDidChange".into(),
            data: json!({"line": 10}),
        })
    }

    #[test]
    fn encode_contains_content_length_header() {
        let encoded = encode_message(&sample_request());
        let text = String::from_utf8(encoded).unwrap();
        assert!(text.starts_with("Content-Length: "));
        assert!(text.contains("\r\n\r\n"));
    }

    #[test]
    fn encode_body_is_valid_json() {
        let encoded = encode_message(&sample_request());
        let text = String::from_utf8(encoded).unwrap();
        let body = text.split("\r\n\r\n").nth(1).unwrap();
        let _: serde_json::Value = serde_json::from_str(body).unwrap();
    }

    #[test]
    fn decode_from_encoded_bytes() {
        let encoded = encode_message(&sample_request());
        let mut reader = BufReader::new(encoded.as_slice());
        let decoded = decode_message(&mut reader).unwrap().unwrap();
        assert_eq!(decoded, sample_request());
    }

    #[test]
    fn decode_returns_none_on_eof() {
        let mut reader = BufReader::new(&b""[..]);
        let result = decode_message(&mut reader).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn decode_rejects_missing_content_length() {
        let data = b"\r\n{\"type\":\"request\"}";
        let mut reader = BufReader::new(&data[..]);
        let result = decode_message(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn transport_roundtrip_request() {
        let mut buf = Vec::new();
        // Write
        {
            let msg = sample_request();
            let encoded = encode_message(&msg);
            buf.extend_from_slice(&encoded);
        }
        // Read
        let mut transport = RpcTransport::new(io::sink(), buf.as_slice());
        let received = transport.recv().unwrap().unwrap();
        assert_eq!(received, sample_request());
    }

    #[test]
    fn transport_roundtrip_response() {
        let buf = encode_message(&sample_response());
        let mut transport = RpcTransport::new(io::sink(), buf.as_slice());
        let received = transport.recv().unwrap().unwrap();
        assert_eq!(received, sample_response());
    }

    #[test]
    fn transport_roundtrip_event() {
        let buf = encode_message(&sample_event());
        let mut transport = RpcTransport::new(io::sink(), buf.as_slice());
        let received = transport.recv().unwrap().unwrap();
        assert_eq!(received, sample_event());
    }

    #[test]
    fn transport_send_then_recv_via_pipe() {
        // Use an in-memory pipe: write into a Vec, then read it back.
        let mut output = Vec::new();
        {
            let mut t = RpcTransport::new(&mut output, io::empty());
            t.send(&sample_request()).unwrap();
            t.send(&sample_response()).unwrap();
        }
        // Now read back
        let mut t = RpcTransport::new(io::sink(), output.as_slice());
        let msg1 = t.recv().unwrap().unwrap();
        let msg2 = t.recv().unwrap().unwrap();
        assert_eq!(msg1, sample_request());
        assert_eq!(msg2, sample_response());
    }

    #[test]
    fn transport_multiple_messages_sequential() {
        let messages = vec![sample_request(), sample_response(), sample_event()];
        let mut buf = Vec::new();
        for m in &messages {
            buf.extend_from_slice(&encode_message(m));
        }
        let mut t = RpcTransport::new(io::sink(), buf.as_slice());
        for expected in &messages {
            let got = t.recv().unwrap().unwrap();
            assert_eq!(&got, expected);
        }
        // Next read should be EOF
        assert!(t.recv().unwrap().is_none());
    }
}
