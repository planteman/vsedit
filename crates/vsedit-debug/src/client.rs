//! DAP client that manages communication with a debug adapter process.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::protocol::*;
use crate::types::*;
use crate::DapError;

/// DAP client that manages a debug adapter process.
pub struct DapClient {
    next_seq: AtomicU64,
    writer: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<DapResponse>>>>,
    _event_tx: mpsc::UnboundedSender<DapEvent>,
    event_rx: Mutex<mpsc::UnboundedReceiver<DapEvent>>,
    child: Mutex<Option<Child>>,
    _reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl std::fmt::Debug for DapClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DapClient")
            .field("next_seq", &self.next_seq)
            .finish_non_exhaustive()
    }
}

impl DapClient {
    /// Spawn a debug adapter as a child process and set up communication.
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self, DapError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| DapError::SpawnFailed(format!("{command}: {e}")))?;

        let stdin = child.stdin.take().ok_or(DapError::NoStdin)?;
        let stdout = child.stdout.take().ok_or(DapError::NoStdout)?;

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<DapResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let reader_pending = pending.clone();
        let reader_event_tx = event_tx.clone();
        let reader_handle = tokio::spawn(async move {
            read_loop(stdout, reader_pending, reader_event_tx).await;
        });

        Ok(Self {
            next_seq: AtomicU64::new(1),
            writer: Arc::new(Mutex::new(Some(stdin))),
            pending,
            _event_tx: event_tx,
            event_rx: Mutex::new(event_rx),
            child: Mutex::new(Some(child)),
            _reader_handle: Mutex::new(Some(reader_handle)),
        })
    }

    fn next_seq(&self) -> u64 {
        self.next_seq.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a DAP request and wait for the response.
    pub async fn send_request(&self, request: DapRequest) -> Result<DapResponse, DapError> {
        let seq = self.next_seq();
        let raw = serde_json::json!({
            "seq": seq,
            "type": "request",
            "command": request.command(),
            "arguments": request.arguments(),
        });

        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(seq, tx);

        self.write_message(&raw).await?;

        let resp = rx.await.map_err(|_| DapError::ResponseChannelClosed)?;
        if !resp.success {
            return Err(DapError::RequestFailed {
                request_seq: resp.request_seq,
                message: resp.message.unwrap_or_else(|| "unknown error".into()),
            });
        }
        Ok(resp)
    }

    /// Receive the next event from the debug adapter.
    pub async fn recv_event(&self) -> Option<DapEvent> {
        self.event_rx.lock().await.recv().await
    }

    /// Register a callback for events (spawns a task).
    ///
    /// The callback receives events by polling the internal event channel.
    /// Only one consumer should be active at a time — either `recv_event()`
    /// or `on_event()`, not both.
    pub fn on_event<F>(self: &Arc<Self>, mut callback: F) -> tokio::task::JoinHandle<()>
    where
        F: FnMut(DapEvent) + Send + 'static,
    {
        let client = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                match client.event_rx.lock().await.recv().await {
                    Some(event) => callback(event),
                    None => break,
                }
            }
        })
    }

    async fn write_message(&self, msg: &impl serde::Serialize) -> Result<(), DapError> {
        let data = encode_message(msg);
        let mut guard = self.writer.lock().await;
        let stdin = guard.as_mut().ok_or(DapError::NoStdin)?;
        stdin.write_all(&data).await.map_err(DapError::Io)?;
        stdin.flush().await.map_err(DapError::Io)?;
        Ok(())
    }

    // ── Convenience methods ──

    /// Send the `initialize` request.
    pub async fn initialize(&self) -> Result<DapResponse, DapError> {
        self.send_request(DapRequest::Initialize {
            client_id: "vsedit".into(),
            client_name: "VSEdit".into(),
        })
        .await
    }

    /// Send the `launch` request.
    pub async fn launch(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&str>,
    ) -> Result<DapResponse, DapError> {
        self.send_request(DapRequest::Launch {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: cwd.map(|s| s.to_string()),
            env: vec![],
            no_debug: false,
        })
        .await
    }

    /// Send the `threads` request and parse the response.
    pub async fn threads(&self) -> Result<Vec<Thread>, DapError> {
        let resp = self.send_request(DapRequest::Threads).await?;
        let threads = resp
            .body
            .as_ref()
            .and_then(|b| b.get("threads"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(Thread::from_dap).collect())
            .unwrap_or_default();
        Ok(threads)
    }

    /// Send the `stackTrace` request and parse the response.
    pub async fn stack_trace(&self, thread_id: u64) -> Result<Vec<StackFrame>, DapError> {
        let resp = self
            .send_request(DapRequest::StackTrace {
                thread_id,
                start_frame: None,
                levels: Some(100),
            })
            .await?;
        let frames = resp
            .body
            .as_ref()
            .and_then(|b| b.get("stackFrames"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(StackFrame::from_dap).collect())
            .unwrap_or_default();
        Ok(frames)
    }

    /// Send the `scopes` request and parse the response.
    pub async fn scopes(&self, frame_id: u64) -> Result<Vec<Scope>, DapError> {
        let resp = self
            .send_request(DapRequest::Scopes { frame_id })
            .await?;
        let scopes = resp
            .body
            .as_ref()
            .and_then(|b| b.get("scopes"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(Scope::from_dap).collect())
            .unwrap_or_default();
        Ok(scopes)
    }

    /// Send the `variables` request and parse the response.
    pub async fn variables(&self, variables_reference: u64) -> Result<Vec<Variable>, DapError> {
        let resp = self
            .send_request(DapRequest::Variables {
                variables_reference,
                start: None,
                count: None,
            })
            .await?;
        let vars = resp
            .body
            .as_ref()
            .and_then(|b| b.get("variables"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(Variable::from_dap).collect())
            .unwrap_or_default();
        Ok(vars)
    }

    /// Send the `continue` request.
    pub async fn continue_thread(&self, thread_id: u64) -> Result<DapResponse, DapError> {
        self.send_request(DapRequest::Continue { thread_id }).await
    }

    /// Send the `next` (step over) request.
    pub async fn step_over(&self, thread_id: u64) -> Result<DapResponse, DapError> {
        self.send_request(DapRequest::Next { thread_id }).await
    }

    /// Send the `stepIn` request.
    pub async fn step_in(&self, thread_id: u64) -> Result<DapResponse, DapError> {
        self.send_request(DapRequest::StepIn { thread_id }).await
    }

    /// Send the `stepOut` request.
    pub async fn step_out(&self, thread_id: u64) -> Result<DapResponse, DapError> {
        self.send_request(DapRequest::StepOut { thread_id }).await
    }

    /// Send the `pause` request.
    pub async fn pause(&self, thread_id: u64) -> Result<DapResponse, DapError> {
        self.send_request(DapRequest::Pause { thread_id }).await
    }

    /// Send the `disconnect` request and kill the child process.
    pub async fn disconnect(&self) -> Result<(), DapError> {
        let _ = self
            .send_request(DapRequest::Disconnect {
                restart: false,
                terminate_debuggee: true,
            })
            .await;
        self.kill().await;
        Ok(())
    }

    /// Send the `terminate` request.
    pub async fn terminate(&self) -> Result<DapResponse, DapError> {
        self.send_request(DapRequest::Terminate { restart: false })
            .await
    }

    /// Send an `evaluate` request.
    pub async fn evaluate(
        &self,
        expression: &str,
        frame_id: Option<u64>,
    ) -> Result<String, DapError> {
        let resp = self
            .send_request(DapRequest::Evaluate {
                expression: expression.to_string(),
                frame_id,
                context: Some("repl".into()),
            })
            .await?;
        Ok(resp
            .body
            .as_ref()
            .and_then(|b| b.get("result"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    }

    /// Kill the debug adapter child process.
    pub async fn kill(&self) {
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.kill().await;
        }
    }
}

/// Background reader loop for the debug adapter's stdout.
async fn read_loop(
    mut stdout: tokio::process::ChildStdout,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<DapResponse>>>>,
    event_tx: mpsc::UnboundedSender<DapEvent>,
) {
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];

    loop {
        match stdout.read(&mut tmp).await {
            Ok(0) => break, // EOF
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
            }
            Err(_) => break,
        }

        // Process all complete messages in the buffer
        loop {
            match try_decode_message(&buf) {
                Ok(Some((msg, consumed))) => {
                    buf.drain(..consumed);
                    handle_message(msg, &pending, &event_tx).await;
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!("DAP decode error: {e}");
                    break;
                }
            }
        }
    }
}

async fn handle_message(
    msg: Value,
    pending: &Arc<Mutex<HashMap<u64, oneshot::Sender<DapResponse>>>>,
    event_tx: &mpsc::UnboundedSender<DapEvent>,
) {
    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match msg_type {
        "response" => {
            if let Ok(resp) = serde_json::from_value::<DapResponse>(msg) {
                let mut guard = pending.lock().await;
                if let Some(tx) = guard.remove(&resp.request_seq) {
                    let _ = tx.send(resp);
                }
            }
        }
        "event" => {
            let event_name = msg.get("event").and_then(|v| v.as_str()).unwrap_or("");
            let body = msg.get("body");
            if let Some(event) = DapEvent::from_raw(event_name, body) {
                let _ = event_tx.send(event);
            }
        }
        _ => {
            tracing::debug!("unknown DAP message type: {msg_type}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::DapResponse;

    #[test]
    fn dap_client_is_debug() {
        // Just ensure DapClient implements Debug
        let _: fn(&DapClient) -> String = |c| format!("{c:?}");
    }

    #[tokio::test]
    async fn spawn_nonexistent_command() {
        let result = DapClient::spawn("nonexistent_debug_adapter_binary_xyz", &[]).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{err}").contains("nonexistent_debug_adapter_binary_xyz"));
    }

    #[test]
    fn handle_response_parsing() {
        let json = serde_json::json!({
            "seq": 2,
            "type": "response",
            "request_seq": 1,
            "success": true,
            "command": "initialize",
            "body": {"supportsConfigurationDoneRequest": true},
        });
        let resp: DapResponse = serde_json::from_value(json).unwrap();
        assert!(resp.success);
        assert_eq!(resp.request_seq, 1);
        assert_eq!(resp.command, "initialize");
    }

    #[test]
    fn handle_failed_response() {
        let json = serde_json::json!({
            "seq": 3,
            "type": "response",
            "request_seq": 2,
            "success": false,
            "command": "launch",
            "message": "program not found",
        });
        let resp: DapResponse = serde_json::from_value(json).unwrap();
        assert!(!resp.success);
        assert_eq!(resp.message.as_deref(), Some("program not found"));
    }
}
