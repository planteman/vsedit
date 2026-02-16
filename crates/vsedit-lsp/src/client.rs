//! LSP client that manages communication with a language server process.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use lsp_types::*;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::transport::*;
use crate::LspError;

/// Configuration for a language server.
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub language_ids: Vec<String>,
    pub root_patterns: Vec<String>,
}

fn parse_uri(s: &str) -> Result<Uri, LspError> {
    s.parse::<Uri>()
        .map_err(|e| LspError::InvalidUri(e.to_string()))
}

/// LSP client that manages a single language server process.
pub struct LspClient {
    next_id: AtomicU64,
    writer: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    _diagnostics_tx: mpsc::UnboundedSender<PublishDiagnosticsParams>,
    diagnostics_rx: Mutex<mpsc::UnboundedReceiver<PublishDiagnosticsParams>>,
    child: Mutex<Option<Child>>,
    _reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl std::fmt::Debug for LspClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspClient")
            .field("next_id", &self.next_id)
            .finish_non_exhaustive()
    }
}

impl LspClient {
    /// Spawn a language server process and set up communication.
    pub async fn spawn_server(command: &str, args: &[&str]) -> Result<Self, LspError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| LspError::SpawnFailed(format!("{command}: {e}")))?;

        let stdin = child.stdin.take().ok_or(LspError::NoStdin)?;
        let stdout = child.stdout.take().ok_or(LspError::NoStdout)?;

        let (diag_tx, diag_rx) = mpsc::unbounded_channel();
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let reader_pending = pending.clone();
        let reader_diag_tx = diag_tx.clone();
        let reader_handle = tokio::spawn(async move {
            read_loop(stdout, reader_pending, reader_diag_tx).await;
        });

        Ok(Self {
            next_id: AtomicU64::new(1),
            writer: Arc::new(Mutex::new(Some(stdin))),
            pending,
            _diagnostics_tx: diag_tx,
            diagnostics_rx: Mutex::new(diag_rx),
            child: Mutex::new(Some(child)),
            _reader_handle: Mutex::new(Some(reader_handle)),
        })
    }

    fn next_request_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value, LspError> {
        let id = self.next_request_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        self.write_message(&req).await?;

        let resp = rx.await.map_err(|_| LspError::ResponseChannelClosed)?;
        if let Some(err) = resp.error {
            return Err(LspError::ServerError {
                code: err.code,
                message: err.message,
            });
        }
        Ok(resp.result.unwrap_or(Value::Null))
    }

    async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<(), LspError> {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };
        self.write_message(&notif).await
    }

    async fn write_message(&self, msg: &impl serde::Serialize) -> Result<(), LspError> {
        let data = encode_message(msg);
        let mut guard = self.writer.lock().await;
        let stdin = guard.as_mut().ok_or(LspError::NoStdin)?;
        stdin.write_all(&data).await.map_err(LspError::Io)?;
        stdin.flush().await.map_err(LspError::Io)?;
        Ok(())
    }

    /// Send the `initialize` request.
    #[allow(deprecated)] // root_uri is still widely used by servers
    pub async fn initialize(&self, root_uri: &str) -> Result<InitializeResult, LspError> {
        let params = InitializeParams {
            root_uri: Some(parse_uri(root_uri)?),
            capabilities: ClientCapabilities::default(),
            ..Default::default()
        };
        let result = self
            .send_request("initialize", Some(serde_json::to_value(params).unwrap()))
            .await?;

        self.send_notification(
            "initialized",
            Some(serde_json::to_value(InitializedParams {}).unwrap()),
        )
        .await?;

        serde_json::from_value(result).map_err(|e| LspError::DeserializeFailed(e.to_string()))
    }

    /// Send `textDocument/didOpen` notification.
    pub async fn did_open(&self, uri: &str, language_id: &str, text: &str) -> Result<(), LspError> {
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: parse_uri(uri)?,
                language_id: language_id.to_string(),
                version: 1,
                text: text.to_string(),
            },
        };
        self.send_notification(
            "textDocument/didOpen",
            Some(serde_json::to_value(params).unwrap()),
        )
        .await
    }

    /// Send `textDocument/didChange` notification.
    pub async fn did_change(
        &self,
        uri: &str,
        version: i32,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) -> Result<(), LspError> {
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: parse_uri(uri)?,
                version,
            },
            content_changes: changes,
        };
        self.send_notification(
            "textDocument/didChange",
            Some(serde_json::to_value(params).unwrap()),
        )
        .await
    }

    /// Request completions at a position.
    pub async fn completion(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Option<CompletionResponse>, LspError> {
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: parse_uri(uri)?,
                },
                position: Position { line, character },
            },
            context: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let result = self
            .send_request(
                "textDocument/completion",
                Some(serde_json::to_value(params).unwrap()),
            )
            .await?;

        if result.is_null() {
            return Ok(None);
        }
        serde_json::from_value(result)
            .map(Some)
            .map_err(|e| LspError::DeserializeFailed(e.to_string()))
    }

    /// Request hover information at a position.
    pub async fn hover(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Option<Hover>, LspError> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: parse_uri(uri)?,
                },
                position: Position { line, character },
            },
            work_done_progress_params: Default::default(),
        };
        let result = self
            .send_request(
                "textDocument/hover",
                Some(serde_json::to_value(params).unwrap()),
            )
            .await?;

        if result.is_null() {
            return Ok(None);
        }
        serde_json::from_value(result)
            .map(Some)
            .map_err(|e| LspError::DeserializeFailed(e.to_string()))
    }

    /// Request go-to-definition at a position.
    pub async fn definition(
        &self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Option<GotoDefinitionResponse>, LspError> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: parse_uri(uri)?,
                },
                position: Position { line, character },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let result = self
            .send_request(
                "textDocument/definition",
                Some(serde_json::to_value(params).unwrap()),
            )
            .await?;

        if result.is_null() {
            return Ok(None);
        }
        serde_json::from_value(result)
            .map(Some)
            .map_err(|e| LspError::DeserializeFailed(e.to_string()))
    }

    /// Receive the next diagnostics notification from the server.
    pub async fn recv_diagnostics(&self) -> Option<PublishDiagnosticsParams> {
        self.diagnostics_rx.lock().await.recv().await
    }

    /// Shut down the language server gracefully.
    pub async fn shutdown(&self) -> Result<(), LspError> {
        let _ = self.send_request("shutdown", None).await;
        self.send_notification("exit", None).await?;

        // Drop stdin to signal EOF
        *self.writer.lock().await = None;

        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.wait().await;
        }
        Ok(())
    }
}

async fn read_loop(
    mut stdout: tokio::process::ChildStdout,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    diag_tx: mpsc::UnboundedSender<PublishDiagnosticsParams>,
) {
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];

    loop {
        match stdout.read(&mut tmp).await {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(_) => break,
        }

        loop {
            match try_decode_message(&buf) {
                Ok(Some((msg, consumed))) => {
                    let rest = buf[consumed..].to_vec();
                    buf = rest;
                    dispatch_message(msg, &pending, &diag_tx).await;
                }
                Ok(None) => break,
                Err(_) => {
                    buf.clear();
                    break;
                }
            }
        }
    }
}

async fn dispatch_message(
    msg: JsonRpcMessage,
    pending: &Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    diag_tx: &mpsc::UnboundedSender<PublishDiagnosticsParams>,
) {
    if msg.is_response() {
        if let Some(id) = msg.id {
            let resp = JsonRpcResponse {
                jsonrpc: msg.jsonrpc,
                id: Some(id),
                result: msg.result,
                error: msg.error,
            };
            if let Some(tx) = pending.lock().await.remove(&id) {
                let _ = tx.send(resp);
            }
        }
    } else if msg.is_notification() {
        if msg.method.as_deref() == Some("textDocument/publishDiagnostics") {
            if let Some(params) = msg.params {
                if let Ok(diag) = serde_json::from_value::<PublishDiagnosticsParams>(params) {
                    let _ = diag_tx.send(diag);
                }
            }
        }
    }
}
