//! Extension host RPC protocol

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

// ── Wire protocol message types ──

#[derive(Debug, Clone, PartialEq)]
pub enum RpcMessage {
    Request(RpcRequest),
    Response(RpcResponse),
    Event(RpcEvent),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcRequest {
    pub id: u64,
    pub proxy_id: String,
    pub method: String,
    pub args: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcResponse {
    pub id: u64,
    pub result: Result<serde_json::Value, RpcError>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcEvent {
    pub proxy_id: String,
    pub event_name: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
}

// ── JSON serialization helpers ──

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WireMessage {
    #[serde(rename = "request")]
    Request {
        id: u64,
        #[serde(rename = "proxyId")]
        proxy_id: String,
        method: String,
        args: Vec<serde_json::Value>,
    },
    #[serde(rename = "response")]
    Response {
        id: u64,
        #[serde(flatten)]
        payload: WireResponsePayload,
    },
    #[serde(rename = "event")]
    Event {
        #[serde(rename = "proxyId")]
        proxy_id: String,
        #[serde(rename = "eventName")]
        event_name: String,
        data: serde_json::Value,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum WireResponsePayload {
    Error { error: RpcError },
    Ok { result: serde_json::Value },
}

impl From<&RpcMessage> for WireMessage {
    fn from(msg: &RpcMessage) -> Self {
        match msg {
            RpcMessage::Request(r) => WireMessage::Request {
                id: r.id,
                proxy_id: r.proxy_id.clone(),
                method: r.method.clone(),
                args: r.args.clone(),
            },
            RpcMessage::Response(r) => WireMessage::Response {
                id: r.id,
                payload: match &r.result {
                    Ok(v) => WireResponsePayload::Ok { result: v.clone() },
                    Err(e) => WireResponsePayload::Error { error: e.clone() },
                },
            },
            RpcMessage::Event(e) => WireMessage::Event {
                proxy_id: e.proxy_id.clone(),
                event_name: e.event_name.clone(),
                data: e.data.clone(),
            },
        }
    }
}

impl From<WireMessage> for RpcMessage {
    fn from(wire: WireMessage) -> Self {
        match wire {
            WireMessage::Request {
                id,
                proxy_id,
                method,
                args,
            } => RpcMessage::Request(RpcRequest {
                id,
                proxy_id,
                method,
                args,
            }),
            WireMessage::Response { id, payload } => RpcMessage::Response(RpcResponse {
                id,
                result: match payload {
                    WireResponsePayload::Ok { result } => Ok(result),
                    WireResponsePayload::Error { error } => Err(error),
                },
            }),
            WireMessage::Event {
                proxy_id,
                event_name,
                data,
            } => RpcMessage::Event(RpcEvent {
                proxy_id,
                event_name,
                data,
            }),
        }
    }
}

// ── RpcProtocol ──

pub struct RpcProtocol {
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, tokio::sync::oneshot::Sender<RpcResponse>>>,
}

impl RpcProtocol {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub fn create_request(
        &self,
        proxy_id: &str,
        method: &str,
        args: Vec<serde_json::Value>,
    ) -> (u64, RpcRequest) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = RpcRequest {
            id,
            proxy_id: proxy_id.to_string(),
            method: method.to_string(),
            args,
        };
        (id, req)
    }

    pub fn register_pending(&self, id: u64) -> tokio::sync::oneshot::Receiver<RpcResponse> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        rx
    }

    pub fn resolve_response(&self, response: RpcResponse) {
        if let Some(tx) = self.pending.lock().unwrap().remove(&response.id) {
            let _ = tx.send(response);
        }
    }

    pub fn serialize_message(msg: &RpcMessage) -> String {
        let wire: WireMessage = msg.into();
        serde_json::to_string(&wire).expect("RpcMessage serialization should not fail")
    }

    pub fn deserialize_message(data: &str) -> Result<RpcMessage, serde_json::Error> {
        let wire: WireMessage = serde_json::from_str(data)?;
        Ok(wire.into())
    }
}

impl Default for RpcProtocol {
    fn default() -> Self {
        Self::new()
    }
}

// ── ProxyIdentifier ──

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProxyIdentifier {
    pub id: String,
    pub is_main: bool,
}

impl ProxyIdentifier {
    pub fn main_thread(id: &str) -> Self {
        Self {
            id: id.to_string(),
            is_main: true,
        }
    }

    pub fn ext_host(id: &str) -> Self {
        Self {
            id: id.to_string(),
            is_main: false,
        }
    }
}

// ── Well-known proxy identifiers ──

pub mod proxies {
    pub const MAIN_THREAD_COMMANDS: &str = "MainThreadCommands";
    pub const MAIN_THREAD_CONFIGURATION: &str = "MainThreadConfiguration";
    pub const MAIN_THREAD_DOCUMENTS: &str = "MainThreadDocuments";
    pub const MAIN_THREAD_EDITORS: &str = "MainThreadEditors";
    pub const MAIN_THREAD_LANGUAGES: &str = "MainThreadLanguageFeatures";
    pub const MAIN_THREAD_WINDOW: &str = "MainThreadWindow";
    pub const MAIN_THREAD_WORKSPACE: &str = "MainThreadWorkspace";
    pub const MAIN_THREAD_FILE_SYSTEM: &str = "MainThreadFileSystem";
    pub const MAIN_THREAD_TERMINAL: &str = "MainThreadTerminal";
    pub const MAIN_THREAD_SCM: &str = "MainThreadSCM";
    pub const MAIN_THREAD_DEBUG: &str = "MainThreadDebugService";

    pub const EXT_HOST_COMMANDS: &str = "ExtHostCommands";
    pub const EXT_HOST_DOCUMENTS: &str = "ExtHostDocuments";
    pub const EXT_HOST_EDITORS: &str = "ExtHostTextEditors";
    pub const EXT_HOST_LANGUAGES: &str = "ExtHostLanguageFeatures";
    pub const EXT_HOST_WORKSPACE: &str = "ExtHostWorkspace";
    pub const EXT_HOST_CONFIGURATION: &str = "ExtHostConfiguration";
    pub const EXT_HOST_FILE_SYSTEM: &str = "ExtHostFileSystem";
    pub const EXT_HOST_TERMINAL: &str = "ExtHostTerminal";
    pub const EXT_HOST_SCM: &str = "ExtHostSCM";
    pub const EXT_HOST_DEBUG: &str = "ExtHostDebugService";
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Serialization roundtrips ──

    #[test]
    fn request_roundtrip() {
        let msg = RpcMessage::Request(RpcRequest {
            id: 1,
            proxy_id: "MainThreadCommands".into(),
            method: "executeCommand".into(),
            args: vec![json!("workbench.action.files.save")],
        });
        let serialized = RpcProtocol::serialize_message(&msg);
        let deserialized = RpcProtocol::deserialize_message(&serialized).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn response_ok_roundtrip() {
        let msg = RpcMessage::Response(RpcResponse {
            id: 42,
            result: Ok(json!({"key": "value"})),
        });
        let serialized = RpcProtocol::serialize_message(&msg);
        let deserialized = RpcProtocol::deserialize_message(&serialized).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn response_null_roundtrip() {
        let msg = RpcMessage::Response(RpcResponse {
            id: 1,
            result: Ok(serde_json::Value::Null),
        });
        let serialized = RpcProtocol::serialize_message(&msg);
        let deserialized = RpcProtocol::deserialize_message(&serialized).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn response_error_roundtrip() {
        let msg = RpcMessage::Response(RpcResponse {
            id: 5,
            result: Err(RpcError {
                message: "not found".into(),
                name: Some("NotFoundError".into()),
                stack: Some("at line 10".into()),
            }),
        });
        let serialized = RpcProtocol::serialize_message(&msg);
        let deserialized = RpcProtocol::deserialize_message(&serialized).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn event_roundtrip() {
        let msg = RpcMessage::Event(RpcEvent {
            proxy_id: "ExtHostTextEditors".into(),
            event_name: "onDidChangeTextEditorSelection".into(),
            data: json!({"lineNumber": 10}),
        });
        let serialized = RpcProtocol::serialize_message(&msg);
        let deserialized = RpcProtocol::deserialize_message(&serialized).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn request_json_format() {
        let msg = RpcMessage::Request(RpcRequest {
            id: 1,
            proxy_id: "MainThreadCommands".into(),
            method: "executeCommand".into(),
            args: vec![json!("workbench.action.files.save")],
        });
        let s = RpcProtocol::serialize_message(&msg);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "request");
        assert_eq!(v["id"], 1);
        assert_eq!(v["proxyId"], "MainThreadCommands");
        assert_eq!(v["method"], "executeCommand");
    }

    #[test]
    fn response_json_format() {
        let msg = RpcMessage::Response(RpcResponse {
            id: 1,
            result: Ok(serde_json::Value::Null),
        });
        let s = RpcProtocol::serialize_message(&msg);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "response");
        assert_eq!(v["id"], 1);
        assert!(v.get("result").is_some());
    }

    #[test]
    fn event_json_format() {
        let msg = RpcMessage::Event(RpcEvent {
            proxy_id: "ExtHostTextEditors".into(),
            event_name: "onDidChangeTextEditorSelection".into(),
            data: json!({}),
        });
        let s = RpcProtocol::serialize_message(&msg);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "event");
        assert_eq!(v["proxyId"], "ExtHostTextEditors");
        assert_eq!(v["eventName"], "onDidChangeTextEditorSelection");
    }

    // ── Request ID generation ──

    #[test]
    fn request_ids_are_sequential() {
        let proto = RpcProtocol::new();
        let (id1, _) = proto.create_request("Svc", "m", vec![]);
        let (id2, _) = proto.create_request("Svc", "m", vec![]);
        let (id3, _) = proto.create_request("Svc", "m", vec![]);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn create_request_populates_fields() {
        let proto = RpcProtocol::new();
        let (id, req) = proto.create_request("MainThreadCommands", "exec", vec![json!(42)]);
        assert_eq!(req.id, id);
        assert_eq!(req.proxy_id, "MainThreadCommands");
        assert_eq!(req.method, "exec");
        assert_eq!(req.args, vec![json!(42)]);
    }

    // ── Response correlation ──

    #[tokio::test]
    async fn response_correlation() {
        let proto = RpcProtocol::new();
        let (id, _req) = proto.create_request("Svc", "method", vec![]);
        let rx = proto.register_pending(id);

        let response = RpcResponse {
            id,
            result: Ok(json!("done")),
        };
        proto.resolve_response(response.clone());

        let received = rx.await.unwrap();
        assert_eq!(received, response);
    }

    #[tokio::test]
    async fn resolve_unknown_id_does_not_panic() {
        let proto = RpcProtocol::new();
        proto.resolve_response(RpcResponse {
            id: 999,
            result: Ok(json!(null)),
        });
    }

    // ── Proxy identifiers ──

    #[test]
    fn proxy_main_thread() {
        let p = ProxyIdentifier::main_thread("MainThreadCommands");
        assert_eq!(p.id, "MainThreadCommands");
        assert!(p.is_main);
    }

    #[test]
    fn proxy_ext_host() {
        let p = ProxyIdentifier::ext_host("ExtHostCommands");
        assert_eq!(p.id, "ExtHostCommands");
        assert!(!p.is_main);
    }

    #[test]
    fn well_known_proxies() {
        assert_eq!(proxies::MAIN_THREAD_COMMANDS, "MainThreadCommands");
        assert_eq!(proxies::EXT_HOST_COMMANDS, "ExtHostCommands");
        assert_eq!(proxies::MAIN_THREAD_DEBUG, "MainThreadDebugService");
        assert_eq!(proxies::EXT_HOST_DEBUG, "ExtHostDebugService");
    }
}
