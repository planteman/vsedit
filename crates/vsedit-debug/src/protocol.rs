//! DAP protocol types: messages, requests, events, and responses.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Wire-level message framing (Content-Length, same as LSP)
// ---------------------------------------------------------------------------

/// Encode a JSON value into a Content-Length framed message.
pub fn encode_message(value: &impl Serialize) -> Vec<u8> {
    let body = serde_json::to_string(value).expect("serialize DAP message");
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut out = Vec::with_capacity(header.len() + body.len());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(body.as_bytes());
    out
}

/// Try to extract a complete Content-Length framed message from a buffer.
///
/// Returns `Ok(Some((value, consumed)))` on success, `Ok(None)` if not
/// enough data is available, or `Err` on parse failure.
pub fn try_decode_message(buf: &[u8]) -> std::io::Result<Option<(Value, usize)>> {
    let header_end = match buf.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(pos) => pos,
        None => return Ok(None),
    };

    let header = std::str::from_utf8(&buf[..header_end])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let content_length = parse_content_length(header).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing Content-Length")
    })?;

    let body_start = header_end + 4;
    let total = body_start + content_length;

    if buf.len() < total {
        return Ok(None);
    }

    let body = &buf[body_start..total];
    let msg: Value = serde_json::from_slice(body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    Ok(Some((msg, total)))
}

fn parse_content_length(header: &str) -> Option<usize> {
    for line in header.split("\r\n") {
        if let Some(val) = line.trim().strip_prefix("Content-Length:") {
            return val.trim().parse().ok();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// DAP message types
// ---------------------------------------------------------------------------

/// A DAP protocol message — either a Request, Response, or Event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DapMessage {
    Request(DapRawRequest),
    Response(DapResponse),
    Event(DapRawEvent),
}

/// A raw DAP request (wire format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapRawRequest {
    pub seq: u64,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

/// A raw DAP event (wire format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapRawEvent {
    pub seq: u64,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

/// A DAP response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapResponse {
    pub seq: u64,
    pub request_seq: u64,
    pub success: bool,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

// ---------------------------------------------------------------------------
// Typed request variants
// ---------------------------------------------------------------------------

/// High-level DAP request variants.
#[derive(Debug, Clone, PartialEq)]
pub enum DapRequest {
    Initialize {
        client_id: String,
        client_name: String,
    },
    Launch {
        program: String,
        args: Vec<String>,
        cwd: Option<String>,
        env: Vec<(String, String)>,
        no_debug: bool,
    },
    Attach {
        port: Option<u16>,
        process_id: Option<u32>,
    },
    SetBreakpoints {
        source_path: String,
        breakpoints: Vec<SourceBreakpoint>,
    },
    Continue {
        thread_id: u64,
    },
    Next {
        thread_id: u64,
    },
    StepIn {
        thread_id: u64,
    },
    StepOut {
        thread_id: u64,
    },
    Pause {
        thread_id: u64,
    },
    Terminate {
        restart: bool,
    },
    Disconnect {
        restart: bool,
        terminate_debuggee: bool,
    },
    Threads,
    StackTrace {
        thread_id: u64,
        start_frame: Option<u32>,
        levels: Option<u32>,
    },
    Scopes {
        frame_id: u64,
    },
    Variables {
        variables_reference: u64,
        start: Option<u32>,
        count: Option<u32>,
    },
    Evaluate {
        expression: String,
        frame_id: Option<u64>,
        context: Option<String>,
    },
    Source {
        source_reference: u64,
    },
}

/// A source breakpoint sent in SetBreakpoints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceBreakpoint {
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_message: Option<String>,
}

impl DapRequest {
    /// Return the DAP command string for this request.
    pub fn command(&self) -> &'static str {
        match self {
            DapRequest::Initialize { .. } => "initialize",
            DapRequest::Launch { .. } => "launch",
            DapRequest::Attach { .. } => "attach",
            DapRequest::SetBreakpoints { .. } => "setBreakpoints",
            DapRequest::Continue { .. } => "continue",
            DapRequest::Next { .. } => "next",
            DapRequest::StepIn { .. } => "stepIn",
            DapRequest::StepOut { .. } => "stepOut",
            DapRequest::Pause { .. } => "pause",
            DapRequest::Terminate { .. } => "terminate",
            DapRequest::Disconnect { .. } => "disconnect",
            DapRequest::Threads => "threads",
            DapRequest::StackTrace { .. } => "stackTrace",
            DapRequest::Scopes { .. } => "scopes",
            DapRequest::Variables { .. } => "variables",
            DapRequest::Evaluate { .. } => "evaluate",
            DapRequest::Source { .. } => "source",
        }
    }

    /// Serialize request arguments to JSON.
    pub fn arguments(&self) -> Option<Value> {
        match self {
            DapRequest::Initialize {
                client_id,
                client_name,
            } => Some(serde_json::json!({
                "clientID": client_id,
                "clientName": client_name,
                "adapterID": "vsedit",
                "linesStartAt1": true,
                "columnsStartAt1": true,
                "pathFormat": "path",
                "supportsVariableType": true,
                "supportsVariablePaging": true,
                "supportsRunInTerminalRequest": false,
            })),
            DapRequest::Launch {
                program,
                args,
                cwd,
                env,
                no_debug,
            } => {
                let mut obj = serde_json::json!({
                    "program": program,
                    "args": args,
                    "noDebug": no_debug,
                });
                if let Some(cwd) = cwd {
                    obj["cwd"] = Value::String(cwd.clone());
                }
                if !env.is_empty() {
                    let env_obj: serde_json::Map<String, Value> = env
                        .iter()
                        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                        .collect();
                    obj["env"] = Value::Object(env_obj);
                }
                Some(obj)
            }
            DapRequest::Attach { port, process_id } => {
                let mut obj = serde_json::json!({});
                if let Some(p) = port {
                    obj["port"] = Value::Number((*p).into());
                }
                if let Some(pid) = process_id {
                    obj["processId"] = Value::Number((*pid).into());
                }
                Some(obj)
            }
            DapRequest::SetBreakpoints {
                source_path,
                breakpoints,
            } => Some(serde_json::json!({
                "source": { "path": source_path },
                "breakpoints": breakpoints,
            })),
            DapRequest::Continue { thread_id } => {
                Some(serde_json::json!({ "threadId": thread_id }))
            }
            DapRequest::Next { thread_id } => {
                Some(serde_json::json!({ "threadId": thread_id }))
            }
            DapRequest::StepIn { thread_id } => {
                Some(serde_json::json!({ "threadId": thread_id }))
            }
            DapRequest::StepOut { thread_id } => {
                Some(serde_json::json!({ "threadId": thread_id }))
            }
            DapRequest::Pause { thread_id } => {
                Some(serde_json::json!({ "threadId": thread_id }))
            }
            DapRequest::Terminate { restart } => {
                Some(serde_json::json!({ "restart": restart }))
            }
            DapRequest::Disconnect {
                restart,
                terminate_debuggee,
            } => Some(serde_json::json!({
                "restart": restart,
                "terminateDebuggee": terminate_debuggee,
            })),
            DapRequest::Threads => None,
            DapRequest::StackTrace {
                thread_id,
                start_frame,
                levels,
            } => {
                let mut obj = serde_json::json!({ "threadId": thread_id });
                if let Some(s) = start_frame {
                    obj["startFrame"] = Value::Number((*s).into());
                }
                if let Some(l) = levels {
                    obj["levels"] = Value::Number((*l).into());
                }
                Some(obj)
            }
            DapRequest::Scopes { frame_id } => {
                Some(serde_json::json!({ "frameId": frame_id }))
            }
            DapRequest::Variables {
                variables_reference,
                start,
                count,
            } => {
                let mut obj = serde_json::json!({ "variablesReference": variables_reference });
                if let Some(s) = start {
                    obj["start"] = Value::Number((*s).into());
                }
                if let Some(c) = count {
                    obj["count"] = Value::Number((*c).into());
                }
                Some(obj)
            }
            DapRequest::Evaluate {
                expression,
                frame_id,
                context,
            } => {
                let mut obj = serde_json::json!({ "expression": expression });
                if let Some(fid) = frame_id {
                    obj["frameId"] = Value::Number((*fid).into());
                }
                if let Some(ctx) = context {
                    obj["context"] = Value::String(ctx.clone());
                }
                Some(obj)
            }
            DapRequest::Source { source_reference } => {
                Some(serde_json::json!({ "sourceReference": source_reference }))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Typed event variants
// ---------------------------------------------------------------------------

/// High-level DAP event variants.
#[derive(Debug, Clone, PartialEq)]
pub enum DapEvent {
    Initialized,
    Stopped {
        reason: String,
        thread_id: Option<u64>,
        all_threads_stopped: bool,
    },
    Continued {
        thread_id: u64,
        all_threads_continued: bool,
    },
    Exited {
        exit_code: i64,
    },
    Terminated {
        restart: bool,
    },
    Thread {
        reason: String,
        thread_id: u64,
    },
    Output {
        category: String,
        output: String,
    },
    Breakpoint {
        reason: String,
        breakpoint_id: Option<u64>,
        verified: bool,
        line: Option<u32>,
    },
    Module {
        reason: String,
        module_id: String,
        module_name: String,
    },
    LoadedSource {
        reason: String,
        source_path: Option<String>,
    },
    Process {
        name: String,
        system_process_id: Option<u64>,
        is_local_process: bool,
    },
    Capabilities,
}

impl DapEvent {
    /// Parse a DAP event from event name and body.
    pub fn from_raw(event: &str, body: Option<&Value>) -> Option<Self> {
        let empty = Value::Object(Default::default());
        let body = body.unwrap_or(&empty);
        match event {
            "initialized" => Some(DapEvent::Initialized),
            "stopped" => Some(DapEvent::Stopped {
                reason: body.get("reason")?.as_str()?.to_string(),
                thread_id: body.get("threadId").and_then(|v| v.as_u64()),
                all_threads_stopped: body
                    .get("allThreadsStopped")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            }),
            "continued" => Some(DapEvent::Continued {
                thread_id: body.get("threadId")?.as_u64()?,
                all_threads_continued: body
                    .get("allThreadsContinued")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            }),
            "exited" => Some(DapEvent::Exited {
                exit_code: body.get("exitCode")?.as_i64()?,
            }),
            "terminated" => Some(DapEvent::Terminated {
                restart: body
                    .get("restart")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            }),
            "thread" => Some(DapEvent::Thread {
                reason: body.get("reason")?.as_str()?.to_string(),
                thread_id: body.get("threadId")?.as_u64()?,
            }),
            "output" => Some(DapEvent::Output {
                category: body
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("console")
                    .to_string(),
                output: body.get("output")?.as_str()?.to_string(),
            }),
            "breakpoint" => {
                let bp = body.get("breakpoint")?;
                Some(DapEvent::Breakpoint {
                    reason: body.get("reason")?.as_str()?.to_string(),
                    breakpoint_id: bp.get("id").and_then(|v| v.as_u64()),
                    verified: bp
                        .get("verified")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    line: bp.get("line").and_then(|v| v.as_u64()).map(|v| v as u32),
                })
            }
            "module" => {
                let m = body.get("module")?;
                Some(DapEvent::Module {
                    reason: body.get("reason")?.as_str()?.to_string(),
                    module_id: m.get("id")?.to_string(),
                    module_name: m.get("name")?.as_str()?.to_string(),
                })
            }
            "loadedSource" => Some(DapEvent::LoadedSource {
                reason: body.get("reason")?.as_str()?.to_string(),
                source_path: body
                    .get("source")
                    .and_then(|s| s.get("path"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            }),
            "process" => Some(DapEvent::Process {
                name: body.get("name")?.as_str()?.to_string(),
                system_process_id: body.get("systemProcessId").and_then(|v| v.as_u64()),
                is_local_process: body
                    .get("isLocalProcess")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
            }),
            "capabilities" => Some(DapEvent::Capabilities),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let req = DapRawRequest {
            seq: 1,
            command: "initialize".to_string(),
            arguments: Some(serde_json::json!({"clientID": "vsedit"})),
        };
        let msg = serde_json::json!({
            "type": "request",
            "seq": req.seq,
            "command": req.command,
            "arguments": req.arguments,
        });
        let encoded = encode_message(&msg);
        let (decoded, consumed) = try_decode_message(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded["seq"], 1);
        assert_eq!(decoded["command"], "initialize");
    }

    #[test]
    fn partial_message_returns_none() {
        let msg = serde_json::json!({"seq": 1});
        let encoded = encode_message(&msg);
        let partial = &encoded[..encoded.len() - 5];
        assert!(try_decode_message(partial).unwrap().is_none());
    }

    #[test]
    fn no_header_returns_none() {
        assert!(try_decode_message(b"partial data").unwrap().is_none());
    }

    #[test]
    fn request_command_names() {
        assert_eq!(
            DapRequest::Initialize {
                client_id: "vsedit".into(),
                client_name: "VSEdit".into()
            }
            .command(),
            "initialize"
        );
        assert_eq!(DapRequest::Threads.command(), "threads");
        assert_eq!(
            DapRequest::Continue { thread_id: 1 }.command(),
            "continue"
        );
        assert_eq!(
            DapRequest::SetBreakpoints {
                source_path: "main.rs".into(),
                breakpoints: vec![]
            }
            .command(),
            "setBreakpoints"
        );
    }

    #[test]
    fn request_arguments_initialize() {
        let req = DapRequest::Initialize {
            client_id: "vsedit".into(),
            client_name: "VSEdit".into(),
        };
        let args = req.arguments().unwrap();
        assert_eq!(args["clientID"], "vsedit");
        assert_eq!(args["linesStartAt1"], true);
    }

    #[test]
    fn request_arguments_launch() {
        let req = DapRequest::Launch {
            program: "/bin/test".into(),
            args: vec!["--flag".into()],
            cwd: Some("/tmp".into()),
            env: vec![("FOO".into(), "bar".into())],
            no_debug: false,
        };
        let args = req.arguments().unwrap();
        assert_eq!(args["program"], "/bin/test");
        assert_eq!(args["cwd"], "/tmp");
        assert_eq!(args["env"]["FOO"], "bar");
    }

    #[test]
    fn request_arguments_threads_is_none() {
        assert!(DapRequest::Threads.arguments().is_none());
    }

    #[test]
    fn request_arguments_set_breakpoints() {
        let req = DapRequest::SetBreakpoints {
            source_path: "src/main.rs".into(),
            breakpoints: vec![SourceBreakpoint {
                line: 10,
                column: None,
                condition: Some("x > 5".into()),
                hit_condition: None,
                log_message: None,
            }],
        };
        let args = req.arguments().unwrap();
        assert_eq!(args["source"]["path"], "src/main.rs");
        assert_eq!(args["breakpoints"][0]["line"], 10);
        assert_eq!(args["breakpoints"][0]["condition"], "x > 5");
    }

    #[test]
    fn request_arguments_evaluate() {
        let req = DapRequest::Evaluate {
            expression: "x + 1".into(),
            frame_id: Some(42),
            context: Some("repl".into()),
        };
        let args = req.arguments().unwrap();
        assert_eq!(args["expression"], "x + 1");
        assert_eq!(args["frameId"], 42);
        assert_eq!(args["context"], "repl");
    }

    #[test]
    fn event_parse_initialized() {
        let event = DapEvent::from_raw("initialized", None);
        assert_eq!(event, Some(DapEvent::Initialized));
    }

    #[test]
    fn event_parse_stopped() {
        let body = serde_json::json!({
            "reason": "breakpoint",
            "threadId": 1,
            "allThreadsStopped": true,
        });
        let event = DapEvent::from_raw("stopped", Some(&body)).unwrap();
        assert_eq!(
            event,
            DapEvent::Stopped {
                reason: "breakpoint".into(),
                thread_id: Some(1),
                all_threads_stopped: true,
            }
        );
    }

    #[test]
    fn event_parse_exited() {
        let body = serde_json::json!({"exitCode": 0});
        let event = DapEvent::from_raw("exited", Some(&body)).unwrap();
        assert_eq!(event, DapEvent::Exited { exit_code: 0 });
    }

    #[test]
    fn event_parse_output() {
        let body = serde_json::json!({"category": "stdout", "output": "hello\n"});
        let event = DapEvent::from_raw("output", Some(&body)).unwrap();
        assert_eq!(
            event,
            DapEvent::Output {
                category: "stdout".into(),
                output: "hello\n".into(),
            }
        );
    }

    #[test]
    fn event_parse_unknown_returns_none() {
        assert!(DapEvent::from_raw("unknownEvent", None).is_none());
    }

    #[test]
    fn event_parse_terminated() {
        let body = serde_json::json!({"restart": false});
        let event = DapEvent::from_raw("terminated", Some(&body)).unwrap();
        assert_eq!(event, DapEvent::Terminated { restart: false });
    }

    #[test]
    fn event_parse_thread() {
        let body = serde_json::json!({"reason": "started", "threadId": 7});
        let event = DapEvent::from_raw("thread", Some(&body)).unwrap();
        assert_eq!(
            event,
            DapEvent::Thread {
                reason: "started".into(),
                thread_id: 7,
            }
        );
    }

    #[test]
    fn event_parse_process() {
        let body = serde_json::json!({
            "name": "my_app",
            "systemProcessId": 12345,
            "isLocalProcess": true,
        });
        let event = DapEvent::from_raw("process", Some(&body)).unwrap();
        assert_eq!(
            event,
            DapEvent::Process {
                name: "my_app".into(),
                system_process_id: Some(12345),
                is_local_process: true,
            }
        );
    }

    #[test]
    fn event_parse_breakpoint() {
        let body = serde_json::json!({
            "reason": "changed",
            "breakpoint": { "id": 1, "verified": true, "line": 42 },
        });
        let event = DapEvent::from_raw("breakpoint", Some(&body)).unwrap();
        assert_eq!(
            event,
            DapEvent::Breakpoint {
                reason: "changed".into(),
                breakpoint_id: Some(1),
                verified: true,
                line: Some(42),
            }
        );
    }

    #[test]
    fn dap_response_serde() {
        let resp = DapResponse {
            seq: 2,
            request_seq: 1,
            success: true,
            command: "initialize".into(),
            message: None,
            body: Some(serde_json::json!({"supportsConfigurationDoneRequest": true})),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: DapResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.request_seq, 1);
        assert!(back.success);
    }

    #[test]
    fn source_breakpoint_serde() {
        let bp = SourceBreakpoint {
            line: 42,
            column: Some(5),
            condition: Some("x > 0".into()),
            hit_condition: None,
            log_message: Some("hit line 42".into()),
        };
        let json = serde_json::to_string(&bp).unwrap();
        let back: SourceBreakpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(back.line, 42);
        assert_eq!(back.column, Some(5));
        assert_eq!(back.log_message.as_deref(), Some("hit line 42"));
    }

    #[test]
    fn request_arguments_attach() {
        let req = DapRequest::Attach {
            port: Some(9229),
            process_id: None,
        };
        let args = req.arguments().unwrap();
        assert_eq!(args["port"], 9229);
    }

    #[test]
    fn request_arguments_stack_trace() {
        let req = DapRequest::StackTrace {
            thread_id: 1,
            start_frame: Some(0),
            levels: Some(20),
        };
        let args = req.arguments().unwrap();
        assert_eq!(args["threadId"], 1);
        assert_eq!(args["startFrame"], 0);
        assert_eq!(args["levels"], 20);
    }

    #[test]
    fn request_arguments_variables() {
        let req = DapRequest::Variables {
            variables_reference: 100,
            start: Some(0),
            count: Some(50),
        };
        let args = req.arguments().unwrap();
        assert_eq!(args["variablesReference"], 100);
    }

    #[test]
    fn request_arguments_disconnect() {
        let req = DapRequest::Disconnect {
            restart: false,
            terminate_debuggee: true,
        };
        let args = req.arguments().unwrap();
        assert_eq!(args["terminateDebuggee"], true);
    }
}
