//! Debug Adapter Protocol (DAP) client and debugging support.
//!
//! Implements the DAP specification for communicating with debug adapters,
//! breakpoint management, call stack inspection, variable viewing, debug
//! console, and launch configuration parsing.

pub mod breakpoints;
pub mod client;
pub mod console;
pub mod launch;
pub mod protocol;
pub mod types;

use std::collections::HashMap;
use std::fmt;
pub use breakpoints::{BreakpointStore, Breakpoint};
pub use client::DapClient;
pub use console::{DebugConsole, DebugConsoleEntry, OutputCategory};
pub use launch::{LaunchConfig, parse_launch_json};
pub use protocol::{DapMessage, DapRequest, DapEvent, DapResponse};
pub use types::{StackFrame, Thread, Variable, Scope, StoppedReason};

/// Errors produced by the debug subsystem.
#[derive(Debug, thiserror::Error)]
pub enum DapError {
    #[error("failed to spawn debug adapter: {0}")]
    SpawnFailed(String),
    #[error("adapter stdin not available")]
    NoStdin,
    #[error("adapter stdout not available")]
    NoStdout,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("response channel closed")]
    ResponseChannelClosed,
    #[error("adapter error: {message}")]
    AdapterError { message: String },
    #[error("request failed (seq {request_seq}): {message}")]
    RequestFailed { request_seq: u64, message: String },
    #[error("failed to deserialize: {0}")]
    DeserializeFailed(String),
    #[error("invalid launch configuration: {0}")]
    InvalidConfig(String),
    #[error("session not active")]
    NotActive,
    #[error("invalid state transition: cannot {action} while {state}")]
    InvalidTransition { action: String, state: String },
}

// ---------------------------------------------------------------------------
// DebugSessionState
// ---------------------------------------------------------------------------

/// High-level state of a debug session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DebugSessionState {
    NotStarted,
    Initializing,
    Running,
    Paused,
    Stopped,
    Terminated,
}

impl std::fmt::Display for DebugSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted => write!(f, "Not Started"),
            Self::Initializing => write!(f, "Initializing"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Stopped => write!(f, "Stopped"),
            Self::Terminated => write!(f, "Terminated"),
        }
    }
}

impl DebugSessionState {
    /// Returns `true` if the session is in a state where execution commands
    /// (pause, step, continue) are meaningful.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }

    /// Returns `true` if the session has ended and cannot be resumed.
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Stopped | Self::Terminated)
    }
}

// ---------------------------------------------------------------------------
// DebugSessionInfo
// ---------------------------------------------------------------------------

/// Read-only snapshot of a debug session's metadata.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DebugSessionInfo {
    pub session_id: String,
    pub name: String,
    pub state: DebugSessionState,
    pub adapter_type: String,
    pub start_time_ms: u64,
    pub breakpoint_count: usize,
}

impl DebugSessionInfo {
    /// Human-readable one-line summary.
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} ({}) — {} — {} breakpoints",
            self.session_id, self.name, self.adapter_type, self.state, self.breakpoint_count,
        )
    }
}

// ---------------------------------------------------------------------------
// DebugSession
// ---------------------------------------------------------------------------

/// Manages the lifecycle of a single debug session.
///
/// Tracks state transitions and validates that commands are legal in the
/// current state (e.g. you cannot pause a stopped session).
#[derive(Debug)]
pub struct DebugSession {
    pub id: String,
    pub name: String,
    pub adapter_type: String,
    state: DebugSessionState,
    start_time_ms: u64,
    breakpoint_count: usize,
}

impl DebugSession {
    /// Create a new session in the `NotStarted` state.
    pub fn new(id: impl Into<String>, name: impl Into<String>, adapter_type: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            adapter_type: adapter_type.into(),
            state: DebugSessionState::NotStarted,
            start_time_ms: 0,
            breakpoint_count: 0,
        }
    }

    pub fn state(&self) -> DebugSessionState {
        self.state
    }

    pub fn set_breakpoint_count(&mut self, count: usize) {
        self.breakpoint_count = count;
    }

    /// Return a read-only snapshot of this session.
    pub fn info(&self) -> DebugSessionInfo {
        DebugSessionInfo {
            session_id: self.id.clone(),
            name: self.name.clone(),
            state: self.state,
            adapter_type: self.adapter_type.clone(),
            start_time_ms: self.start_time_ms,
            breakpoint_count: self.breakpoint_count,
        }
    }

    // -- lifecycle transitions ------------------------------------------------

    /// Move to `Initializing`. Only valid from `NotStarted`.
    pub fn initialize(&mut self) -> Result<(), DapError> {
        self.require_state(&[DebugSessionState::NotStarted], "initialize")?;
        self.state = DebugSessionState::Initializing;
        tracing::info!(session = %self.id, "session initializing");
        Ok(())
    }

    /// Transition from `Initializing` to `Running` via a launch request.
    pub fn launch(&mut self, start_time_ms: u64) -> Result<(), DapError> {
        self.require_state(&[DebugSessionState::Initializing], "launch")?;
        self.start_time_ms = start_time_ms;
        self.state = DebugSessionState::Running;
        tracing::info!(session = %self.id, "session launched");
        Ok(())
    }

    /// Transition from `Initializing` to `Running` via an attach request.
    pub fn attach(&mut self, start_time_ms: u64) -> Result<(), DapError> {
        self.require_state(&[DebugSessionState::Initializing], "attach")?;
        self.start_time_ms = start_time_ms;
        self.state = DebugSessionState::Running;
        tracing::info!(session = %self.id, "session attached");
        Ok(())
    }

    /// Pause a running session.
    pub fn pause(&mut self) -> Result<(), DapError> {
        self.require_state(&[DebugSessionState::Running], "pause")?;
        self.state = DebugSessionState::Paused;
        tracing::debug!(session = %self.id, "session paused");
        Ok(())
    }

    /// Continue (resume) a paused session.
    pub fn continue_execution(&mut self) -> Result<(), DapError> {
        self.require_state(&[DebugSessionState::Paused], "continue")?;
        self.state = DebugSessionState::Running;
        tracing::debug!(session = %self.id, "session continued");
        Ok(())
    }

    /// Step over — only valid when paused; remains paused after the step.
    pub fn step_over(&mut self) -> Result<(), DapError> {
        self.require_state(&[DebugSessionState::Paused], "step_over")?;
        tracing::debug!(session = %self.id, "step over");
        Ok(())
    }

    /// Step into — only valid when paused; remains paused after the step.
    pub fn step_into(&mut self) -> Result<(), DapError> {
        self.require_state(&[DebugSessionState::Paused], "step_into")?;
        tracing::debug!(session = %self.id, "step into");
        Ok(())
    }

    /// Step out — only valid when paused; remains paused after the step.
    pub fn step_out(&mut self) -> Result<(), DapError> {
        self.require_state(&[DebugSessionState::Paused], "step_out")?;
        tracing::debug!(session = %self.id, "step out");
        Ok(())
    }

    /// Terminate the debuggee. Valid from `Running` or `Paused`.
    pub fn terminate(&mut self) -> Result<(), DapError> {
        self.require_state(
            &[DebugSessionState::Running, DebugSessionState::Paused],
            "terminate",
        )?;
        self.state = DebugSessionState::Terminated;
        tracing::info!(session = %self.id, "session terminated");
        Ok(())
    }

    /// Disconnect from the adapter. Valid from any active or initializing state.
    pub fn disconnect(&mut self) -> Result<(), DapError> {
        self.require_state(
            &[
                DebugSessionState::Initializing,
                DebugSessionState::Running,
                DebugSessionState::Paused,
            ],
            "disconnect",
        )?;
        self.state = DebugSessionState::Stopped;
        tracing::info!(session = %self.id, "session disconnected");
        Ok(())
    }

    // -- helpers --------------------------------------------------------------

    fn require_state(&self, allowed: &[DebugSessionState], action: &str) -> Result<(), DapError> {
        if allowed.contains(&self.state) {
            Ok(())
        } else {
            Err(DapError::InvalidTransition {
                action: action.to_string(),
                state: self.state.to_string(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// BreakpointSummary
// ---------------------------------------------------------------------------

/// Aggregate counts of breakpoints by type.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BreakpointSummary {
    pub line: usize,
    pub function: usize,
    pub data: usize,
    pub exception: usize,
}

impl BreakpointSummary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Total breakpoints across all types.
    pub fn total(&self) -> usize {
        self.line + self.function + self.data + self.exception
    }

    /// Merge another summary into this one.
    pub fn merge(&mut self, other: &BreakpointSummary) {
        self.line += other.line;
        self.function += other.function;
        self.data += other.data;
        self.exception += other.exception;
    }

    /// Human-readable label.
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.line > 0 {
            parts.push(format!("{} line", self.line));
        }
        if self.function > 0 {
            parts.push(format!("{} function", self.function));
        }
        if self.data > 0 {
            parts.push(format!("{} data", self.data));
        }
        if self.exception > 0 {
            parts.push(format!("{} exception", self.exception));
        }
        if parts.is_empty() {
            "no breakpoints".to_string()
        } else {
            parts.join(", ")
        }
    }
}

impl std::fmt::Display for BreakpointSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ---------------------------------------------------------------------------
// DebugCapabilities
// ---------------------------------------------------------------------------

/// Capabilities reported by a debug adapter during initialization.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DebugCapabilities {
    pub supports_configuration_done: bool,
    pub supports_restart: bool,
    pub supports_stepping_granularity: bool,
    pub supports_function_breakpoints: bool,
    pub supports_conditional_breakpoints: bool,
    pub supports_hit_conditional_breakpoints: bool,
    pub supports_log_points: bool,
    pub supports_data_breakpoints: bool,
    pub supports_evaluate_for_hovers: bool,
    pub supports_set_variable: bool,
    pub supports_terminate_request: bool,
    pub supports_loaded_sources: bool,
    pub supports_exception_info: bool,
    pub supports_completions: bool,
}

impl Default for DebugCapabilities {
    fn default() -> Self {
        Self {
            supports_configuration_done: true,
            supports_restart: false,
            supports_stepping_granularity: false,
            supports_function_breakpoints: false,
            supports_conditional_breakpoints: false,
            supports_hit_conditional_breakpoints: false,
            supports_log_points: false,
            supports_data_breakpoints: false,
            supports_evaluate_for_hovers: false,
            supports_set_variable: false,
            supports_terminate_request: false,
            supports_loaded_sources: false,
            supports_exception_info: false,
            supports_completions: false,
        }
    }
}

impl DebugCapabilities {
    /// Parse capabilities from a DAP `initialize` response body.
    pub fn from_dap(body: &serde_json::Value) -> Self {
        let b = |key: &str| -> bool {
            body.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
        };
        Self {
            supports_configuration_done: b("supportsConfigurationDoneRequest"),
            supports_restart: b("supportsRestartRequest"),
            supports_stepping_granularity: b("supportsSteppingGranularity"),
            supports_function_breakpoints: b("supportsFunctionBreakpoints"),
            supports_conditional_breakpoints: b("supportsConditionalBreakpoints"),
            supports_hit_conditional_breakpoints: b("supportsHitConditionalBreakpoints"),
            supports_log_points: b("supportsLogPoints"),
            supports_data_breakpoints: b("supportsDataBreakpoints"),
            supports_evaluate_for_hovers: b("supportsEvaluateForHovers"),
            supports_set_variable: b("supportsSetVariable"),
            supports_terminate_request: b("supportsTerminateRequest"),
            supports_loaded_sources: b("supportsLoadedSourcesRequest"),
            supports_exception_info: b("supportsExceptionInfoRequest"),
            supports_completions: b("supportsCompletionsRequest"),
        }
    }

    /// Return a list of human-readable capability names that are enabled.
    pub fn enabled_list(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.supports_configuration_done { out.push("configurationDone"); }
        if self.supports_restart { out.push("restart"); }
        if self.supports_stepping_granularity { out.push("steppingGranularity"); }
        if self.supports_function_breakpoints { out.push("functionBreakpoints"); }
        if self.supports_conditional_breakpoints { out.push("conditionalBreakpoints"); }
        if self.supports_hit_conditional_breakpoints { out.push("hitConditionalBreakpoints"); }
        if self.supports_log_points { out.push("logPoints"); }
        if self.supports_data_breakpoints { out.push("dataBreakpoints"); }
        if self.supports_evaluate_for_hovers { out.push("evaluateForHovers"); }
        if self.supports_set_variable { out.push("setVariable"); }
        if self.supports_terminate_request { out.push("terminateRequest"); }
        if self.supports_loaded_sources { out.push("loadedSources"); }
        if self.supports_exception_info { out.push("exceptionInfo"); }
        if self.supports_completions { out.push("completions"); }
        out
    }
}

// ---------------------------------------------------------------------------
// WatchExpression / WatchStore
// ---------------------------------------------------------------------------

/// A single watch expression and its last evaluated result.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WatchExpression {
    pub id: u64,
    pub expression: String,
    pub last_value: Option<String>,
    pub last_type: Option<String>,
    pub error: Option<String>,
}

impl WatchExpression {
    pub fn new(id: u64, expression: impl Into<String>) -> Self {
        Self {
            id,
            expression: expression.into(),
            last_value: None,
            last_type: None,
            error: None,
        }
    }

    /// Mark this expression as successfully evaluated.
    pub fn set_result(&mut self, value: impl Into<String>, type_name: Option<String>) {
        self.last_value = Some(value.into());
        self.last_type = type_name;
        self.error = None;
    }

    /// Mark this expression as having an evaluation error.
    pub fn set_error(&mut self, err: impl Into<String>) {
        self.last_value = None;
        self.last_type = None;
        self.error = Some(err.into());
    }
}

/// Manages an ordered list of watch expressions.
#[derive(Debug, Default)]
pub struct WatchStore {
    expressions: Vec<WatchExpression>,
    next_id: u64,
}

impl WatchStore {
    pub fn new() -> Self {
        Self {
            expressions: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a new watch expression. Returns its assigned id.
    pub fn add(&mut self, expression: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.expressions.push(WatchExpression::new(id, expression));
        id
    }

    /// Remove a watch expression by id. Returns `true` if found.
    pub fn remove(&mut self, id: u64) -> bool {
        let len_before = self.expressions.len();
        self.expressions.retain(|w| w.id != id);
        self.expressions.len() < len_before
    }

    /// Move the expression at `from` to `to`. Returns `false` if out of bounds.
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.expressions.len() || to >= self.expressions.len() {
            return false;
        }
        let item = self.expressions.remove(from);
        self.expressions.insert(to, item);
        true
    }

    /// Get all watch expressions.
    pub fn expressions(&self) -> &[WatchExpression] {
        &self.expressions
    }

    /// Get a mutable reference by id.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut WatchExpression> {
        self.expressions.iter_mut().find(|w| w.id == id)
    }

    /// Number of watch expressions.
    pub fn len(&self) -> usize {
        self.expressions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.expressions.is_empty()
    }

    /// Evaluate all expressions using a provided closure. The closure receives
    /// the expression string and returns `Ok((value, optional_type))` or
    /// `Err(message)`.
    pub fn evaluate_all<F>(&mut self, mut evaluator: F)
    where
        F: FnMut(&str) -> Result<(String, Option<String>), String>,
    {
        for watch in &mut self.expressions {
            match evaluator(&watch.expression) {
                Ok((val, ty)) => watch.set_result(val, ty),
                Err(e) => watch.set_error(e),
            }
        }
    }

    /// Clear evaluation results from all expressions (e.g. when session stops).
    pub fn clear_results(&mut self) {
        for watch in &mut self.expressions {
            watch.last_value = None;
            watch.last_type = None;
            watch.error = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Validate that a `serde_json::Value` (typically a launch config body)
/// contains all required field names. Returns a list of missing fields.
pub fn validate_launch_config_fields(
    config: &serde_json::Value,
    required: &[&str],
) -> Vec<String> {
    let obj = match config.as_object() {
        Some(o) => o,
        None => return required.iter().map(|s| s.to_string()).collect(),
    };
    required
        .iter()
        .filter(|&&key| {
            match obj.get(key) {
                None => true,
                Some(v) => v.is_null(),
            }
        })
        .map(|s| s.to_string())
        .collect()
}

/// Format a raw memory address as a `0x`-prefixed hex string with zero-padding
/// appropriate for 64-bit addresses (16 hex digits).
pub fn format_memory_address(addr: u64) -> String {
    format!("0x{:016X}", addr)
}

/// Parsed components of a single stack trace line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedStackFrame {
    pub frame_number: Option<u32>,
    pub function_name: String,
    pub file_path: Option<String>,
    pub line: Option<u32>,
}

/// Parse a single line from a textual stack trace.
///
/// Supports common formats:
/// - `#0 main at /src/main.rs:42`
/// - `#1 0x00007fff foo::bar at lib.rs:10`
/// - `foo::bar (/path/to/file.rs:10)`
///
/// Returns `None` if the line cannot be parsed at all.
pub fn parse_stack_trace_line(line: &str) -> Option<ParsedStackFrame> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Format: #N [addr] name [at file:line]
    if line.starts_with('#') {
        return parse_gdb_style_frame(line);
    }

    // Format: name (file:line)
    if let Some(paren_start) = line.rfind('(') {
        if line.ends_with(')') {
            let func_part = line[..paren_start].trim();
            let loc = &line[paren_start + 1..line.len() - 1];
            let (file, ln) = split_file_line(loc);
            return Some(ParsedStackFrame {
                frame_number: None,
                function_name: func_part.to_string(),
                file_path: Some(file.to_string()),
                line: ln,
            });
        }
    }

    // Bare function name
    Some(ParsedStackFrame {
        frame_number: None,
        function_name: line.to_string(),
        file_path: None,
        line: None,
    })
}

/// Parse GDB/LLDB-style `#N ...` frame lines.
fn parse_gdb_style_frame(line: &str) -> Option<ParsedStackFrame> {
    let rest = &line[1..]; // skip '#'
    let mut parts = rest.splitn(2, char::is_whitespace);
    let frame_num: u32 = parts.next()?.trim().parse().ok()?;
    let remainder = parts.next()?.trim();

    // Skip optional hex address (0x...)
    let remainder = if remainder.starts_with("0x") {
        remainder
            .splitn(2, char::is_whitespace)
            .nth(1)
            .unwrap_or("")
            .trim()
    } else {
        remainder
    };

    // Split on " at " to separate function from location
    if let Some(at_pos) = remainder.find(" at ") {
        let func = &remainder[..at_pos];
        let loc = &remainder[at_pos + 4..];
        let (file, ln) = split_file_line(loc);
        Some(ParsedStackFrame {
            frame_number: Some(frame_num),
            function_name: func.trim().to_string(),
            file_path: Some(file.to_string()),
            line: ln,
        })
    } else {
        Some(ParsedStackFrame {
            frame_number: Some(frame_num),
            function_name: remainder.to_string(),
            file_path: None,
            line: None,
        })
    }
}

/// Split `file:line` into `(file, Option<line>)`.
fn split_file_line(s: &str) -> (&str, Option<u32>) {
    if let Some(colon) = s.rfind(':') {
        let (file, rest) = s.split_at(colon);
        let ln = rest[1..].parse().ok();
        (file, ln)
    } else {
        (s, None)
    }
}

// ---------------------------------------------------------------------------
// DebugAdapterMessage
// ---------------------------------------------------------------------------

/// A typed representation of a DAP protocol message (request, response, or event).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DebugAdapterMessage {
    #[serde(rename = "request")]
    Request {
        seq: u64,
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        arguments: Option<serde_json::Value>,
    },
    #[serde(rename = "response")]
    Response {
        seq: u64,
        request_seq: u64,
        success: bool,
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    #[serde(rename = "event")]
    Event {
        seq: u64,
        event: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<serde_json::Value>,
    },
}

impl DebugAdapterMessage {
    /// Returns `true` if this message is a request.
    pub fn is_request(&self) -> bool {
        matches!(self, Self::Request { .. })
    }

    /// Returns `true` if this message is a response.
    pub fn is_response(&self) -> bool {
        matches!(self, Self::Response { .. })
    }

    /// Returns `true` if this message is an event.
    pub fn is_event(&self) -> bool {
        matches!(self, Self::Event { .. })
    }

    /// Returns the sequence number of the message.
    pub fn seq(&self) -> u64 {
        match self {
            Self::Request { seq, .. }
            | Self::Response { seq, .. }
            | Self::Event { seq, .. } => *seq,
        }
    }

    /// Returns the command name (for request/response) or event name.
    pub fn command_or_event(&self) -> &str {
        match self {
            Self::Request { command, .. } | Self::Response { command, .. } => command,
            Self::Event { event, .. } => event,
        }
    }
}

// ---------------------------------------------------------------------------
// DebugSessionLifecycle
// ---------------------------------------------------------------------------

/// Tracks debug session state transitions with timestamps.
#[derive(Debug, Clone)]
pub struct DebugSessionLifecycle {
    pub session_id: String,
    transitions: Vec<(DebugSessionState, u64)>,
}

impl DebugSessionLifecycle {
    /// Create a new lifecycle tracker for the given session.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            transitions: Vec::new(),
        }
    }

    /// Record a state transition at the given timestamp (milliseconds).
    pub fn record_transition(&mut self, state: DebugSessionState, timestamp_ms: u64) {
        self.transitions.push((state, timestamp_ms));
    }

    /// Returns the most recently recorded state, or `NotStarted` if empty.
    pub fn current_state(&self) -> DebugSessionState {
        self.transitions
            .last()
            .map(|(s, _)| *s)
            .unwrap_or(DebugSessionState::NotStarted)
    }

    /// Returns the number of recorded transitions.
    pub fn transition_count(&self) -> usize {
        self.transitions.len()
    }

    /// Compute total time (ms) spent in the given state across all intervals.
    ///
    /// Each transition marks the *start* of time in that state; the duration
    /// lasts until the next transition (or is open-ended for the last entry,
    /// which is excluded from the sum).
    pub fn time_in_state(&self, state: DebugSessionState) -> Option<u64> {
        let mut total: u64 = 0;
        let mut found = false;
        for window in self.transitions.windows(2) {
            if window[0].0 == state {
                found = true;
                total += window[1].1.saturating_sub(window[0].1);
            }
        }
        if found { Some(total) } else { None }
    }

    /// Returns the full transition history.
    pub fn history(&self) -> &[(DebugSessionState, u64)] {
        &self.transitions
    }
}

// ---------------------------------------------------------------------------
// debug_capabilities_negotiate
// ---------------------------------------------------------------------------

/// Negotiate debug capabilities by merging an adapter's initialize response
/// with the client's requested capabilities.
///
/// `_client_caps` is the client's `InitializeRequestArguments` (currently
/// unused but reserved for future negotiation logic).
/// `adapter_caps` is the body of the adapter's `initialize` response.
pub fn debug_capabilities_negotiate(
    _client_caps: &serde_json::Value,
    adapter_caps: &serde_json::Value,
) -> DebugCapabilities {
    DebugCapabilities::from_dap(adapter_caps)
}

// ---------------------------------------------------------------------------
// DebugAdapterMessageCodec
// ---------------------------------------------------------------------------

/// Codec for encoding / decoding DAP messages with Content-Length framing.
pub struct DebugAdapterMessageCodec;

impl DebugAdapterMessageCodec {
    /// Encode a `DebugAdapterMessage` into the DAP wire format:
    ///
    /// ```text
    /// Content-Length: <len>\r\n\r\n<json>
    /// ```
    pub fn encode(msg: &DebugAdapterMessage) -> String {
        let json = serde_json::to_string(msg).expect("DebugAdapterMessage is always serialisable");
        format!("Content-Length: {}\r\n\r\n{}", json.len(), json)
    }

    /// Decode a DAP wire-format payload into a `DebugAdapterMessage`.
    ///
    /// Expects `raw` to start with a `Content-Length:` header followed by
    /// `\r\n\r\n` and the JSON body.
    pub fn decode(raw: &str) -> Result<DebugAdapterMessage, DapError> {
        let separator = "\r\n\r\n";
        let sep_pos = raw.find(separator).ok_or_else(|| {
            DapError::DeserializeFailed("missing header/body separator".into())
        })?;
        let header = &raw[..sep_pos];
        let body = &raw[sep_pos + separator.len()..];

        // Validate Content-Length header is present.
        let _content_length: usize = header
            .strip_prefix("Content-Length: ")
            .and_then(|v| v.trim().parse().ok())
            .ok_or_else(|| {
                DapError::DeserializeFailed("invalid Content-Length header".into())
            })?;

        serde_json::from_str(body).map_err(|e| DapError::DeserializeFailed(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// DAP message parsing helpers
// ---------------------------------------------------------------------------

/// Extract the command name from a raw DAP JSON string.
pub fn extract_command_from_json(json_str: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    v.get("command")
        .or_else(|| v.get("event"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extract the sequence number from a raw DAP JSON string.
pub fn extract_seq_from_json(json_str: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    v.get("seq").and_then(|v| v.as_u64())
}

/// Check if a DAP JSON message is a success response.
pub fn is_success_response(json_str: &str) -> bool {
    let v: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return false,
    };
    v.get("type").and_then(|t| t.as_str()) == Some("response")
        && v.get("success").and_then(|s| s.as_bool()) == Some(true)
}

// ---------------------------------------------------------------------------
// Breakpoint management utilities
// ---------------------------------------------------------------------------

/// A managed collection of breakpoints indexed by file path.
#[derive(Debug, Clone, Default)]
pub struct BreakpointManager {
    breakpoints: std::collections::HashMap<String, Vec<u32>>,
}

impl BreakpointManager {
    /// Create an empty breakpoint manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle a breakpoint at the given file and line.
    ///
    /// If a breakpoint already exists at that location, it is removed.
    /// Otherwise, a new breakpoint is added.
    pub fn toggle(&mut self, file: &str, line: u32) -> bool {
        let lines = self.breakpoints.entry(file.to_string()).or_default();
        if let Some(pos) = lines.iter().position(|&l| l == line) {
            lines.remove(pos);
            false // removed
        } else {
            lines.push(line);
            lines.sort_unstable();
            true // added
        }
    }

    /// Get all breakpoint lines for a file.
    pub fn get_lines(&self, file: &str) -> &[u32] {
        self.breakpoints.get(file).map_or(&[], |v| v.as_slice())
    }

    /// Check if a breakpoint exists at the given file and line.
    pub fn has_breakpoint(&self, file: &str, line: u32) -> bool {
        self.breakpoints
            .get(file)
            .map_or(false, |lines| lines.contains(&line))
    }

    /// Total number of breakpoints across all files.
    pub fn total_count(&self) -> usize {
        self.breakpoints.values().map(|v| v.len()).sum()
    }

    /// Number of files that have breakpoints.
    pub fn file_count(&self) -> usize {
        self.breakpoints.values().filter(|v| !v.is_empty()).count()
    }

    /// Clear all breakpoints.
    pub fn clear(&mut self) {
        self.breakpoints.clear();
    }

    /// Remove all breakpoints for a specific file.
    pub fn clear_file(&mut self, file: &str) {
        self.breakpoints.remove(file);
    }
}

// ---------------------------------------------------------------------------
// Variable formatting
// ---------------------------------------------------------------------------

/// Format a variable for display in the debug variables panel.
pub fn format_variable_value(name: &str, value: &str, type_name: Option<&str>) -> String {
    match type_name {
        Some(tn) => format!("{name}: {tn} = {value}"),
        None => format!("{name} = {value}"),
    }
}

/// Truncate a variable value for display, adding "…" if truncated.
pub fn truncate_variable_value(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    if max_len <= 1 {
        return "…".to_string();
    }
    let mut s: String = value.chars().take(max_len - 1).collect();
    s.push('…');
    s
}

/// Format a collection of variables as a multi-line summary.
pub fn format_variables_summary(vars: &[(String, String, Option<String>)]) -> String {
    let mut lines = Vec::with_capacity(vars.len());
    for (name, value, type_name) in vars {
        lines.push(format_variable_value(name, value, type_name.as_deref()));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Stack frame navigation helpers
// ---------------------------------------------------------------------------

/// Navigate up in a stack trace, returning the index of the previous frame.
pub fn navigate_stack_up(current_index: usize, total_frames: usize) -> usize {
    if total_frames == 0 || current_index == 0 {
        return current_index;
    }
    current_index - 1
}

/// Navigate down in a stack trace, returning the index of the next frame.
pub fn navigate_stack_down(current_index: usize, total_frames: usize) -> usize {
    if total_frames == 0 || current_index >= total_frames - 1 {
        return current_index;
    }
    current_index + 1
}

/// Find a frame in the stack by function name prefix.
pub fn find_frame_by_function(
    frames: &[ParsedStackFrame],
    function_prefix: &str,
) -> Option<usize> {
    frames.iter().position(|f| f.function_name.starts_with(function_prefix))
}

// ---------------------------------------------------------------------------
// DebugSession — additional methods
// ---------------------------------------------------------------------------

impl DebugSession {
    /// Returns `true` if the session is active (running or paused).
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Returns `true` if the session has ended.
    pub fn is_finished(&self) -> bool {
        self.state.is_finished()
    }

    /// Returns the elapsed time since launch, given the current timestamp.
    pub fn elapsed_ms(&self, current_time_ms: u64) -> u64 {
        if self.start_time_ms == 0 {
            return 0;
        }
        current_time_ms.saturating_sub(self.start_time_ms)
    }
}

// ---------------------------------------------------------------------------
// BreakpointManager — additional methods
// ---------------------------------------------------------------------------

impl BreakpointManager {
    /// Returns all files that have at least one breakpoint.
    pub fn files(&self) -> Vec<&str> {
        self.breakpoints
            .iter()
            .filter(|(_, lines)| !lines.is_empty())
            .map(|(file, _)| file.as_str())
            .collect()
    }

    /// Adds a breakpoint at the given file and line (no-op if already exists).
    /// Returns `true` if the breakpoint was newly added.
    pub fn add(&mut self, file: &str, line: u32) -> bool {
        let lines = self.breakpoints.entry(file.to_string()).or_default();
        if lines.contains(&line) {
            false
        } else {
            lines.push(line);
            lines.sort_unstable();
            true
        }
    }

    /// Removes a breakpoint at the given file and line.
    /// Returns `true` if the breakpoint existed and was removed.
    pub fn remove(&mut self, file: &str, line: u32) -> bool {
        if let Some(lines) = self.breakpoints.get_mut(file) {
            if let Some(pos) = lines.iter().position(|&l| l == line) {
                lines.remove(pos);
                return true;
            }
        }
        false
    }

    /// Returns a summary of breakpoints per file.
    pub fn summary(&self) -> Vec<(&str, usize)> {
        let mut result: Vec<(&str, usize)> = self
            .breakpoints
            .iter()
            .filter(|(_, lines)| !lines.is_empty())
            .map(|(file, lines)| (file.as_str(), lines.len()))
            .collect();
        result.sort_by_key(|(file, _)| file.to_string());
        result
    }
}

// ---------------------------------------------------------------------------
// WatchStore — additional methods
// ---------------------------------------------------------------------------

impl WatchStore {
    /// Returns `true` if any expression has an error.
    pub fn has_errors(&self) -> bool {
        self.expressions.iter().any(|w| w.error.is_some())
    }

    /// Returns expressions that have errors.
    pub fn errored_expressions(&self) -> Vec<&WatchExpression> {
        self.expressions.iter().filter(|w| w.error.is_some()).collect()
    }

    /// Removes all expressions.
    pub fn clear(&mut self) {
        self.expressions.clear();
    }

    /// Returns a reference to an expression by id.
    pub fn get(&self, id: u64) -> Option<&WatchExpression> {
        self.expressions.iter().find(|w| w.id == id)
    }
}

// ---------------------------------------------------------------------------
// DebugSessionState — additional methods
// ---------------------------------------------------------------------------

impl DebugSessionState {
    /// Returns `true` if the session can be initialized from this state.
    pub fn can_initialize(&self) -> bool {
        matches!(self, Self::NotStarted)
    }

    /// Returns `true` if stepping is allowed in this state.
    pub fn can_step(&self) -> bool {
        matches!(self, Self::Paused)
    }
}

// ---------------------------------------------------------------------------
// Stepping granularity
// ---------------------------------------------------------------------------

/// DAP stepping granularity for step-in/step-out/step-over requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SteppingGranularity {
    Statement,
    Line,
    Instruction,
}

impl fmt::Display for SteppingGranularity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Statement => write!(f, "statement"),
            Self::Line => write!(f, "line"),
            Self::Instruction => write!(f, "instruction"),
        }
    }
}

impl SteppingGranularity {
    /// Parse a granularity string from a DAP message.
    pub fn from_dap_str(s: &str) -> Option<Self> {
        match s {
            "statement" => Some(Self::Statement),
            "line" => Some(Self::Line),
            "instruction" => Some(Self::Instruction),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Exception filter configuration
// ---------------------------------------------------------------------------

/// Configuration for a single exception filter (e.g. "uncaught", "raised").
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExceptionFilterConfig {
    pub filter_id: String,
    pub label: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub condition: Option<String>,
}

impl ExceptionFilterConfig {
    pub fn new(filter_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            filter_id: filter_id.into(),
            label: label.into(),
            description: None,
            enabled: true,
            condition: None,
        }
    }

    /// Create from a DAP `ExceptionBreakpointsFilter` JSON value.
    pub fn from_dap(value: &serde_json::Value) -> Option<Self> {
        let filter_id = value.get("filter")?.as_str()?.to_string();
        let label = value.get("label")?.as_str()?.to_string();
        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let default_enabled = value
            .get("default")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let condition = value
            .get("conditionDescription")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Some(Self {
            filter_id,
            label,
            description,
            enabled: default_enabled,
            condition,
        })
    }
}

/// Manages a set of exception filter configurations.
#[derive(Debug, Clone, Default)]
pub struct ExceptionFilterStore {
    filters: Vec<ExceptionFilterConfig>,
}

impl ExceptionFilterStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Populate from the `exceptionBreakpointFilters` array in the DAP
    /// `initialize` response body.
    pub fn load_from_dap(&mut self, body: &serde_json::Value) {
        if let Some(arr) = body.get("exceptionBreakpointFilters").and_then(|v| v.as_array()) {
            self.filters.clear();
            for item in arr {
                if let Some(f) = ExceptionFilterConfig::from_dap(item) {
                    self.filters.push(f);
                }
            }
        }
    }

    /// Toggle the enabled state of a filter by its ID. Returns the new state,
    /// or `None` if the filter was not found.
    pub fn toggle(&mut self, filter_id: &str) -> Option<bool> {
        let f = self.filters.iter_mut().find(|f| f.filter_id == filter_id)?;
        f.enabled = !f.enabled;
        Some(f.enabled)
    }

    /// Set a condition string on an exception filter.
    pub fn set_condition(&mut self, filter_id: &str, condition: Option<String>) -> bool {
        if let Some(f) = self.filters.iter_mut().find(|f| f.filter_id == filter_id) {
            f.condition = condition;
            true
        } else {
            false
        }
    }

    /// Returns the list of currently enabled filter IDs (for the
    /// `setExceptionBreakpoints` request).
    pub fn enabled_ids(&self) -> Vec<&str> {
        self.filters
            .iter()
            .filter(|f| f.enabled)
            .map(|f| f.filter_id.as_str())
            .collect()
    }

    pub fn filters(&self) -> &[ExceptionFilterConfig] {
        &self.filters
    }

    pub fn len(&self) -> usize {
        self.filters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Data breakpoint tracking
// ---------------------------------------------------------------------------

/// Access type for data breakpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DataBreakpointAccessType {
    Read,
    Write,
    ReadWrite,
}

impl fmt::Display for DataBreakpointAccessType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::ReadWrite => write!(f, "readWrite"),
        }
    }
}

/// A data breakpoint that fires on memory/variable access.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DataBreakpoint {
    pub data_id: String,
    pub access_type: DataBreakpointAccessType,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
}

impl DataBreakpoint {
    pub fn new(data_id: impl Into<String>, access_type: DataBreakpointAccessType) -> Self {
        Self {
            data_id: data_id.into(),
            access_type,
            condition: None,
            hit_condition: None,
        }
    }

    /// Serialise to the JSON body expected by `setDataBreakpoints`.
    pub fn to_dap_json(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "dataId": self.data_id,
            "accessType": self.access_type,
        });
        if let Some(c) = &self.condition {
            obj["condition"] = serde_json::Value::String(c.clone());
        }
        if let Some(h) = &self.hit_condition {
            obj["hitCondition"] = serde_json::Value::String(h.clone());
        }
        obj
    }
}

/// Manages a collection of data breakpoints.
#[derive(Debug, Clone, Default)]
pub struct DataBreakpointStore {
    breakpoints: Vec<DataBreakpoint>,
}

impl DataBreakpointStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, bp: DataBreakpoint) {
        if !self.breakpoints.iter().any(|b| b.data_id == bp.data_id) {
            self.breakpoints.push(bp);
        }
    }

    pub fn remove(&mut self, data_id: &str) -> bool {
        let before = self.breakpoints.len();
        self.breakpoints.retain(|b| b.data_id != data_id);
        self.breakpoints.len() < before
    }

    pub fn clear(&mut self) {
        self.breakpoints.clear();
    }

    pub fn breakpoints(&self) -> &[DataBreakpoint] {
        &self.breakpoints
    }

    pub fn len(&self) -> usize {
        self.breakpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.breakpoints.is_empty()
    }

    /// Build the JSON array for the `setDataBreakpoints` request body.
    pub fn to_dap_json(&self) -> serde_json::Value {
        serde_json::Value::Array(self.breakpoints.iter().map(|b| b.to_dap_json()).collect())
    }
}

// ---------------------------------------------------------------------------
// Source reference management
// ---------------------------------------------------------------------------

/// A source reference returned by the debug adapter for decompiled or
/// generated sources that don't exist on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceReference {
    pub reference_id: i64,
    pub name: String,
    pub origin: Option<String>,
    pub content: Option<String>,
}

impl SourceReference {
    pub fn new(reference_id: i64, name: impl Into<String>) -> Self {
        Self {
            reference_id,
            name: name.into(),
            origin: None,
            content: None,
        }
    }

    /// Returns true if the source content has been fetched.
    pub fn is_loaded(&self) -> bool {
        self.content.is_some()
    }

    /// Set the source content after a `source` request.
    pub fn set_content(&mut self, content: impl Into<String>) {
        self.content = Some(content.into());
    }

    /// Line count of loaded content.
    pub fn line_count(&self) -> usize {
        self.content.as_ref().map_or(0, |c| c.lines().count())
    }
}

/// Cache for source references returned by the debug adapter.
#[derive(Debug, Clone, Default)]
pub struct SourceReferenceCache {
    refs: std::collections::HashMap<i64, SourceReference>,
}

impl SourceReferenceCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a source reference (without content). Returns `true` if newly inserted.
    pub fn register(&mut self, reference_id: i64, name: impl Into<String>) -> bool {
        use std::collections::hash_map::Entry;
        match self.refs.entry(reference_id) {
            Entry::Occupied(_) => false,
            Entry::Vacant(e) => {
                e.insert(SourceReference::new(reference_id, name));
                true
            }
        }
    }

    /// Store content for a previously registered source reference.
    pub fn set_content(&mut self, reference_id: i64, content: impl Into<String>) -> bool {
        if let Some(sr) = self.refs.get_mut(&reference_id) {
            sr.set_content(content);
            true
        } else {
            false
        }
    }

    pub fn get(&self, reference_id: i64) -> Option<&SourceReference> {
        self.refs.get(&reference_id)
    }

    /// Returns IDs of references whose content has not been loaded yet.
    pub fn unloaded_ids(&self) -> Vec<i64> {
        self.refs
            .iter()
            .filter(|(_, sr)| !sr.is_loaded())
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.refs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Debug console command parsing
// ---------------------------------------------------------------------------

/// Parsed debug console input — either a meta-command or an expression to evaluate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugConsoleCommand {
    /// An expression to evaluate in the current scope.
    Evaluate(String),
    /// `.bp <file> <line>` — toggle a breakpoint.
    ToggleBreakpoint { file: String, line: u32 },
    /// `.bt` — print backtrace.
    Backtrace,
    /// `.vars` — list local variables.
    ListVariables,
    /// `.threads` — list threads.
    ListThreads,
    /// `.set <var> <value>` — set variable value.
    SetVariable { name: String, value: String },
    /// Unknown dot-command.
    UnknownCommand(String),
}

/// Parse user input from the debug console into a command.
pub fn parse_debug_console_input(input: &str) -> DebugConsoleCommand {
    let input = input.trim();
    if !input.starts_with('.') {
        return DebugConsoleCommand::Evaluate(input.to_string());
    }

    let mut parts = input.splitn(3, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    match cmd {
        ".bp" => {
            let file = parts.next().unwrap_or("").to_string();
            let line: u32 = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            if file.is_empty() || line == 0 {
                DebugConsoleCommand::UnknownCommand(input.to_string())
            } else {
                DebugConsoleCommand::ToggleBreakpoint { file, line }
            }
        }
        ".bt" => DebugConsoleCommand::Backtrace,
        ".vars" => DebugConsoleCommand::ListVariables,
        ".threads" => DebugConsoleCommand::ListThreads,
        ".set" => {
            let name = parts.next().unwrap_or("").to_string();
            let value = parts.next().unwrap_or("").to_string();
            if name.is_empty() {
                DebugConsoleCommand::UnknownCommand(input.to_string())
            } else {
                DebugConsoleCommand::SetVariable { name, value }
            }
        }
        _ => DebugConsoleCommand::UnknownCommand(input.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Breakpoint condition evaluation
// ---------------------------------------------------------------------------

/// A conditional breakpoint descriptor.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConditionalBreakpoint {
    pub file: String,
    pub line: u32,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
    pub log_message: Option<String>,
}

impl ConditionalBreakpoint {
    pub fn new(file: impl Into<String>, line: u32) -> Self {
        Self {
            file: file.into(),
            line,
            condition: None,
            hit_condition: None,
            log_message: None,
        }
    }

    /// Returns `true` if this breakpoint has any condition or log message.
    pub fn is_conditional(&self) -> bool {
        self.condition.is_some() || self.hit_condition.is_some()
    }

    /// Returns `true` if this is a logpoint (has a log message template).
    pub fn is_logpoint(&self) -> bool {
        self.log_message.is_some()
    }

    /// Serialise to the DAP `SourceBreakpoint` JSON format.
    pub fn to_dap_source_breakpoint(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({ "line": self.line });
        if let Some(c) = &self.condition {
            obj["condition"] = serde_json::Value::String(c.clone());
        }
        if let Some(h) = &self.hit_condition {
            obj["hitCondition"] = serde_json::Value::String(h.clone());
        }
        if let Some(l) = &self.log_message {
            obj["logMessage"] = serde_json::Value::String(l.clone());
        }
        obj
    }
}

// ---------------------------------------------------------------------------
// DAP request builder helpers
// ---------------------------------------------------------------------------

/// Sequence number allocator for outgoing DAP requests.
#[derive(Debug)]
pub struct DapSeqAllocator {
    next: u64,
}

impl DapSeqAllocator {
    pub fn new() -> Self {
        Self { next: 1 }
    }

    /// Allocate the next sequence number.
    pub fn next(&mut self) -> u64 {
        let seq = self.next;
        self.next += 1;
        seq
    }

    /// Build a DAP request message with the next sequence number.
    pub fn build_request(
        &mut self,
        command: impl Into<String>,
        arguments: Option<serde_json::Value>,
    ) -> DebugAdapterMessage {
        DebugAdapterMessage::Request {
            seq: self.next(),
            command: command.into(),
            arguments,
        }
    }
}

impl Default for DapSeqAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a DAP `initialize` request body with standard client info.
pub fn build_initialize_args(
    client_id: &str,
    client_name: &str,
    supports_variable_type: bool,
) -> serde_json::Value {
    serde_json::json!({
        "clientID": client_id,
        "clientName": client_name,
        "adapterID": "",
        "linesStartAt1": true,
        "columnsStartAt1": true,
        "pathFormat": "path",
        "supportsVariableType": supports_variable_type,
        "supportsRunInTerminalRequest": false,
    })
}

/// Build a `setBreakpoints` request body from conditional breakpoints for a
/// single source file.
pub fn build_set_breakpoints_body(
    file_path: &str,
    breakpoints: &[ConditionalBreakpoint],
) -> serde_json::Value {
    let bps: Vec<serde_json::Value> = breakpoints
        .iter()
        .map(|b| b.to_dap_source_breakpoint())
        .collect();
    serde_json::json!({
        "source": { "path": file_path },
        "breakpoints": bps,
    })
}

// ---------------------------------------------------------------------------
// Instruction breakpoint management
// ---------------------------------------------------------------------------

/// An instruction breakpoint identified by a memory address.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstructionBreakpoint {
    pub instruction_reference: String,
    pub offset: Option<i64>,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
}

impl InstructionBreakpoint {
    pub fn new(instruction_reference: impl Into<String>) -> Self {
        Self {
            instruction_reference: instruction_reference.into(),
            offset: None,
            condition: None,
            hit_condition: None,
        }
    }

    pub fn to_dap_json(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "instructionReference": self.instruction_reference,
        });
        if let Some(o) = self.offset {
            obj["offset"] = serde_json::json!(o);
        }
        if let Some(c) = &self.condition {
            obj["condition"] = serde_json::Value::String(c.clone());
        }
        if let Some(h) = &self.hit_condition {
            obj["hitCondition"] = serde_json::Value::String(h.clone());
        }
        obj
    }
}

/// Manages a set of instruction breakpoints.
#[derive(Debug, Clone, Default)]
pub struct InstructionBreakpointStore {
    breakpoints: Vec<InstructionBreakpoint>,
}

impl InstructionBreakpointStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, bp: InstructionBreakpoint) {
        if !self
            .breakpoints
            .iter()
            .any(|b| b.instruction_reference == bp.instruction_reference && b.offset == bp.offset)
        {
            self.breakpoints.push(bp);
        }
    }

    pub fn remove(&mut self, instruction_reference: &str) -> bool {
        let before = self.breakpoints.len();
        self.breakpoints
            .retain(|b| b.instruction_reference != instruction_reference);
        self.breakpoints.len() < before
    }

    pub fn clear(&mut self) {
        self.breakpoints.clear();
    }

    pub fn breakpoints(&self) -> &[InstructionBreakpoint] {
        &self.breakpoints
    }

    pub fn len(&self) -> usize {
        self.breakpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.breakpoints.is_empty()
    }

    /// Build the JSON array for the `setInstructionBreakpoints` request body.
    pub fn to_dap_json(&self) -> serde_json::Value {
        serde_json::Value::Array(self.breakpoints.iter().map(|b| b.to_dap_json()).collect())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// debug – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XDebugLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XDebugPanelState {
    pub region: XDebugLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XDebugPanelState {
    pub fn new(region: XDebugLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_debug_total_visible_area(panels: &[XDebugPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_debug_count_in_region(
    panels: &[XDebugPanelState],
    region: XDebugLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_debug_widest_panel(panels: &[XDebugPanelState]) -> Option<&XDebugPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_debug_collapse_region(
    panels: &mut [XDebugPanelState],
    region: XDebugLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XDebugLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XDebugLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}

/// Tracks debug adapter protocol message statistics.
pub struct DapMessageStats {
    requests_sent: u64,
    responses_received: u64,
    events_received: u64,
    errors: u64,
}

impl DapMessageStats {
    pub fn new() -> Self {
        Self { requests_sent: 0, responses_received: 0, events_received: 0, errors: 0 }
    }

    pub fn record_request(&mut self) { self.requests_sent += 1; }
    pub fn record_response(&mut self) { self.responses_received += 1; }
    pub fn record_event(&mut self) { self.events_received += 1; }
    pub fn record_error(&mut self) { self.errors += 1; }

    pub fn requests_sent(&self) -> u64 { self.requests_sent }
    pub fn responses_received(&self) -> u64 { self.responses_received }
    pub fn events_received(&self) -> u64 { self.events_received }
    pub fn errors(&self) -> u64 { self.errors }

    pub fn total_messages(&self) -> u64 {
        self.requests_sent + self.responses_received + self.events_received
    }

    pub fn error_rate(&self) -> f64 {
        let total = self.total_messages();
        if total == 0 { return 0.0; }
        self.errors as f64 / total as f64
    }

    pub fn pending_responses(&self) -> u64 {
        self.requests_sent.saturating_sub(self.responses_received)
    }

    pub fn reset(&mut self) {
        self.requests_sent = 0;
        self.responses_received = 0;
        self.events_received = 0;
        self.errors = 0;
    }
}

/// Manages a stack of debug call frames for display.
pub struct CallFrameStack {
    frames: Vec<(String, String, u32)>, // (name, source, line)
}

impl CallFrameStack {
    pub fn new() -> Self { Self { frames: Vec::new() } }

    pub fn push(&mut self, name: &str, source: &str, line: u32) {
        self.frames.push((name.to_string(), source.to_string(), line));
    }

    pub fn pop(&mut self) -> Option<(String, String, u32)> { self.frames.pop() }

    pub fn depth(&self) -> usize { self.frames.len() }
    pub fn is_empty(&self) -> bool { self.frames.is_empty() }

    pub fn top(&self) -> Option<&(String, String, u32)> { self.frames.last() }

    pub fn frame_at(&self, index: usize) -> Option<&(String, String, u32)> {
        self.frames.get(index)
    }

    pub fn clear(&mut self) { self.frames.clear(); }

    pub fn contains_source(&self, source: &str) -> bool {
        self.frames.iter().any(|(_, s, _)| s == source)
    }

    pub fn sources(&self) -> Vec<String> {
        let mut srcs: Vec<_> = self.frames.iter().map(|(_, s, _)| s.clone()).collect();
        srcs.dedup();
        srcs
    }
}

/// Evaluates simple debug expressions (variable watches).
pub struct DebugWatchEvaluator {
    variables: HashMap<String, String>,
}

impl DebugWatchEvaluator {
    pub fn new() -> Self { Self { variables: HashMap::new() } }

    pub fn set_variable(&mut self, name: &str, value: &str) {
        self.variables.insert(name.to_string(), value.to_string());
    }

    pub fn evaluate(&self, expr: &str) -> Option<String> {
        // Simple variable lookup
        self.variables.get(expr).cloned()
    }

    pub fn variable_count(&self) -> usize { self.variables.len() }

    pub fn remove_variable(&mut self, name: &str) -> bool {
        self.variables.remove(name).is_some()
    }

    pub fn clear(&mut self) { self.variables.clear(); }

    pub fn variable_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.variables.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for debug
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaDebugRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaDebugRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaDebugCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaDebugCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaDebugCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 26
// ---------------------------------------------------------------------------

/// Generic object pool `Xc26Pool<T>`.
pub struct Xc26Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc26Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc26PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc26Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc26PoolStats {
        Xc26PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc26Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc26Scheduler`.
pub struct Xc26Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc26Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc26Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_26 hash for the given byte slice.
pub fn xc_26_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_26 convention.
pub fn xc_26_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe3 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe3Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe3PipelineError {
    pub stage: Xe3Stage,
    pub message: String,
}

impl std::fmt::Display for Xe3PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe3Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe3Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError>>>,
    stage_names: Vec<Xe3Stage>,
}

impl Xe3Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe3Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe3Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe3Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe3Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe3Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe3CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe3CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe3Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe3CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe3CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe3Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe3CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_3_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe3CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_3_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe3CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_3_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError> {
    Ok(data)
}

pub fn xe_3_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_3_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_3_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_3_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe3PipelineError> {
    Err(Xe3PipelineError {
        stage: Xe3Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #64
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf64Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf64TrieNode {
    children: std::collections::HashMap<char, Xf64TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf64Trie {
    root: Xf64TrieNode,
    count: usize,
}

impl Xf64Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf64TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf64TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf64TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf64BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf64BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 25).
pub struct Xh25SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh25SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 67 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 25).
pub struct Xh25BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh25BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 25).
pub struct Xi25Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi25Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi25Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi25Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 25).
pub struct Xi25IntervalTree {
    xi_intervals: Vec<Xi25Interval>,
}

impl Xi25IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi25Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi25Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi25Interval) -> Vec<&Xi25Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi25Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi25Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi25Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi25Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi25Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi25Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 24) ---

/// Disjoint set / union-find for crate 24.
pub struct Xj24UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj24UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ24_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 24.
pub struct Xj24BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj24BTreeNode<K, V>>>,
    len: usize,
}

struct Xj24BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj24BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj24BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ24_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ24_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj24BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj24BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj24BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj24BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_24 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk24SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk24SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk24DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk24DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_24).
#[derive(Debug, Clone)]
pub struct Xl24Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl24Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_24).
#[derive(Debug, Clone)]
pub struct Xl24SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl24SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- DebugSessionState ---------------------------------------------------

    #[test]
    fn session_state_display() {
        assert_eq!(DebugSessionState::NotStarted.to_string(), "Not Started");
        assert_eq!(DebugSessionState::Running.to_string(), "Running");
        assert_eq!(DebugSessionState::Terminated.to_string(), "Terminated");
    }

    #[test]
    fn session_state_is_active() {
        assert!(DebugSessionState::Running.is_active());
        assert!(DebugSessionState::Paused.is_active());
        assert!(!DebugSessionState::NotStarted.is_active());
        assert!(!DebugSessionState::Stopped.is_active());
    }

    #[test]
    fn session_state_is_finished() {
        assert!(DebugSessionState::Stopped.is_finished());
        assert!(DebugSessionState::Terminated.is_finished());
        assert!(!DebugSessionState::Running.is_finished());
    }

    #[test]
    fn session_state_serde_roundtrip() {
        let state = DebugSessionState::Paused;
        let json = serde_json::to_string(&state).unwrap();
        let back: DebugSessionState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, state);
    }

    // -- DebugSessionInfo ----------------------------------------------------

    #[test]
    fn session_info_summary() {
        let info = DebugSessionInfo {
            session_id: "s1".into(),
            name: "MyApp".into(),
            state: DebugSessionState::Running,
            adapter_type: "lldb".into(),
            start_time_ms: 1000,
            breakpoint_count: 3,
        };
        let s = info.summary();
        assert!(s.contains("s1"));
        assert!(s.contains("MyApp"));
        assert!(s.contains("Running"));
        assert!(s.contains("3 breakpoints"));
    }

    // -- DebugSession lifecycle ----------------------------------------------

    #[test]
    fn session_full_lifecycle_launch() {
        let mut s = DebugSession::new("1", "app", "lldb");
        assert_eq!(s.state(), DebugSessionState::NotStarted);

        s.initialize().unwrap();
        assert_eq!(s.state(), DebugSessionState::Initializing);

        s.launch(1000).unwrap();
        assert_eq!(s.state(), DebugSessionState::Running);

        s.pause().unwrap();
        assert_eq!(s.state(), DebugSessionState::Paused);

        s.continue_execution().unwrap();
        assert_eq!(s.state(), DebugSessionState::Running);

        s.pause().unwrap();
        s.terminate().unwrap();
        assert_eq!(s.state(), DebugSessionState::Terminated);
    }

    #[test]
    fn session_attach_lifecycle() {
        let mut s = DebugSession::new("2", "remote", "cppdbg");
        s.initialize().unwrap();
        s.attach(500).unwrap();
        assert_eq!(s.state(), DebugSessionState::Running);
        s.disconnect().unwrap();
        assert_eq!(s.state(), DebugSessionState::Stopped);
    }

    #[test]
    fn session_cannot_pause_when_stopped() {
        let mut s = DebugSession::new("3", "app", "lldb");
        s.initialize().unwrap();
        s.launch(0).unwrap();
        s.disconnect().unwrap();
        let err = s.pause().unwrap_err();
        assert!(err.to_string().contains("Stopped"));
    }

    #[test]
    fn session_cannot_launch_twice() {
        let mut s = DebugSession::new("4", "app", "lldb");
        s.initialize().unwrap();
        s.launch(0).unwrap();
        assert!(s.launch(0).is_err());
    }

    #[test]
    fn session_step_operations_require_paused() {
        let mut s = DebugSession::new("5", "app", "lldb");
        s.initialize().unwrap();
        s.launch(0).unwrap();

        assert!(s.step_over().is_err(), "step_over while running");
        assert!(s.step_into().is_err(), "step_into while running");
        assert!(s.step_out().is_err(), "step_out while running");

        s.pause().unwrap();
        assert!(s.step_over().is_ok());
        assert!(s.step_into().is_ok());
        assert!(s.step_out().is_ok());
        // still paused after stepping
        assert_eq!(s.state(), DebugSessionState::Paused);
    }

    #[test]
    fn session_info_reflects_state() {
        let mut s = DebugSession::new("6", "app", "lldb");
        s.set_breakpoint_count(5);
        s.initialize().unwrap();
        s.launch(42).unwrap();
        let info = s.info();
        assert_eq!(info.state, DebugSessionState::Running);
        assert_eq!(info.start_time_ms, 42);
        assert_eq!(info.breakpoint_count, 5);
    }

    #[test]
    fn session_disconnect_from_initializing() {
        let mut s = DebugSession::new("7", "app", "lldb");
        s.initialize().unwrap();
        assert!(s.disconnect().is_ok());
        assert_eq!(s.state(), DebugSessionState::Stopped);
    }

    // -- BreakpointSummary ---------------------------------------------------

    #[test]
    fn breakpoint_summary_total_and_label() {
        let mut summary = BreakpointSummary::new();
        assert_eq!(summary.total(), 0);
        assert_eq!(summary.label(), "no breakpoints");

        summary.line = 3;
        summary.function = 1;
        assert_eq!(summary.total(), 4);
        assert_eq!(summary.to_string(), "3 line, 1 function");
    }

    #[test]
    fn breakpoint_summary_merge() {
        let mut a = BreakpointSummary { line: 2, function: 0, data: 1, exception: 0 };
        let b = BreakpointSummary { line: 1, function: 3, data: 0, exception: 2 };
        a.merge(&b);
        assert_eq!(a.line, 3);
        assert_eq!(a.function, 3);
        assert_eq!(a.data, 1);
        assert_eq!(a.exception, 2);
    }

    // -- DebugCapabilities ---------------------------------------------------

    #[test]
    fn capabilities_from_dap() {
        let body = serde_json::json!({
            "supportsConfigurationDoneRequest": true,
            "supportsRestartRequest": true,
            "supportsFunctionBreakpoints": true,
            "supportsConditionalBreakpoints": true,
            "supportsEvaluateForHovers": true,
        });
        let caps = DebugCapabilities::from_dap(&body);
        assert!(caps.supports_configuration_done);
        assert!(caps.supports_restart);
        assert!(caps.supports_function_breakpoints);
        assert!(caps.supports_conditional_breakpoints);
        assert!(caps.supports_evaluate_for_hovers);
        assert!(!caps.supports_stepping_granularity);
        assert!(!caps.supports_data_breakpoints);
    }

    #[test]
    fn capabilities_enabled_list() {
        let mut caps = DebugCapabilities::default();
        let list = caps.enabled_list();
        assert_eq!(list, vec!["configurationDone"]); // only default-true

        caps.supports_restart = true;
        caps.supports_completions = true;
        let list = caps.enabled_list();
        assert!(list.contains(&"restart"));
        assert!(list.contains(&"completions"));
    }

    // -- WatchExpression / WatchStore -----------------------------------------

    #[test]
    fn watch_expression_set_result_and_error() {
        let mut w = WatchExpression::new(1, "x + 1");
        w.set_result("42", Some("i32".into()));
        assert_eq!(w.last_value.as_deref(), Some("42"));
        assert_eq!(w.last_type.as_deref(), Some("i32"));
        assert!(w.error.is_none());

        w.set_error("undefined variable");
        assert!(w.last_value.is_none());
        assert!(w.error.as_deref() == Some("undefined variable"));
    }

    #[test]
    fn watch_store_add_remove() {
        let mut store = WatchStore::new();
        assert!(store.is_empty());

        let id1 = store.add("x");
        let id2 = store.add("y + 1");
        assert_eq!(store.len(), 2);

        assert!(store.remove(id1));
        assert_eq!(store.len(), 1);
        assert_eq!(store.expressions()[0].expression, "y + 1");

        assert!(!store.remove(999)); // no-op
        assert_eq!(store.len(), 1);

        store.remove(id2);
        assert!(store.is_empty());
    }

    #[test]
    fn watch_store_reorder() {
        let mut store = WatchStore::new();
        store.add("a");
        store.add("b");
        store.add("c");

        assert!(store.reorder(2, 0)); // move "c" to front
        let names: Vec<&str> = store.expressions().iter().map(|w| w.expression.as_str()).collect();
        assert_eq!(names, vec!["c", "a", "b"]);

        assert!(!store.reorder(0, 10)); // out of bounds
    }

    #[test]
    fn watch_store_evaluate_all() {
        let mut store = WatchStore::new();
        let id1 = store.add("1 + 1");
        let _id2 = store.add("bad_var");

        store.evaluate_all(|expr| {
            if expr == "1 + 1" {
                Ok(("2".into(), Some("i32".into())))
            } else {
                Err("not found".into())
            }
        });

        let w1 = store.get_mut(id1).unwrap();
        assert_eq!(w1.last_value.as_deref(), Some("2"));

        let exprs = store.expressions();
        assert!(exprs[1].error.is_some());
    }

    #[test]
    fn watch_store_clear_results() {
        let mut store = WatchStore::new();
        let id = store.add("x");
        store.get_mut(id).unwrap().set_result("10", None);
        store.clear_results();
        assert!(store.expressions()[0].last_value.is_none());
    }

    // -- Utility functions ---------------------------------------------------

    #[test]
    fn validate_launch_config_fields_missing() {
        let config = serde_json::json!({"name": "Test", "type": "lldb"});
        let missing = validate_launch_config_fields(&config, &["name", "type", "request", "program"]);
        assert_eq!(missing, vec!["request", "program"]);
    }

    #[test]
    fn validate_launch_config_fields_all_present() {
        let config = serde_json::json!({"name": "T", "type": "lldb", "request": "launch"});
        let missing = validate_launch_config_fields(&config, &["name", "type", "request"]);
        assert!(missing.is_empty());
    }

    #[test]
    fn validate_launch_config_fields_null_value() {
        let config = serde_json::json!({"name": "T", "program": null});
        let missing = validate_launch_config_fields(&config, &["name", "program"]);
        assert_eq!(missing, vec!["program"]);
    }

    #[test]
    fn format_memory_address_basic() {
        assert_eq!(format_memory_address(0), "0x0000000000000000");
        assert_eq!(format_memory_address(255), "0x00000000000000FF");
        assert_eq!(format_memory_address(0x7FFF_FFFF_FFFF_FFFF), "0x7FFFFFFFFFFFFFFF");
    }

    #[test]
    fn parse_stack_trace_gdb_style() {
        let frame = parse_stack_trace_line("#0 main at /src/main.rs:42").unwrap();
        assert_eq!(frame.frame_number, Some(0));
        assert_eq!(frame.function_name, "main");
        assert_eq!(frame.file_path.as_deref(), Some("/src/main.rs"));
        assert_eq!(frame.line, Some(42));
    }

    #[test]
    fn parse_stack_trace_with_address() {
        let frame = parse_stack_trace_line("#1 0x00007fff foo::bar at lib.rs:10").unwrap();
        assert_eq!(frame.frame_number, Some(1));
        assert_eq!(frame.function_name, "foo::bar");
        assert_eq!(frame.file_path.as_deref(), Some("lib.rs"));
        assert_eq!(frame.line, Some(10));
    }

    #[test]
    fn parse_stack_trace_paren_style() {
        let frame = parse_stack_trace_line("my_func (/path/to/file.rs:10)").unwrap();
        assert_eq!(frame.frame_number, None);
        assert_eq!(frame.function_name, "my_func");
        assert_eq!(frame.file_path.as_deref(), Some("/path/to/file.rs"));
        assert_eq!(frame.line, Some(10));
    }

    #[test]
    fn parse_stack_trace_bare_function() {
        let frame = parse_stack_trace_line("unknown_func").unwrap();
        assert_eq!(frame.function_name, "unknown_func");
        assert!(frame.file_path.is_none());
        assert!(frame.line.is_none());
    }

    #[test]
    fn parse_stack_trace_empty_returns_none() {
        assert!(parse_stack_trace_line("").is_none());
        assert!(parse_stack_trace_line("   ").is_none());
    }

    #[test]
    fn parse_stack_trace_no_location() {
        let frame = parse_stack_trace_line("#3 some_function").unwrap();
        assert_eq!(frame.frame_number, Some(3));
        assert_eq!(frame.function_name, "some_function");
        assert!(frame.file_path.is_none());
    }

    // -- DebugAdapterMessage -------------------------------------------------

    #[test]
    fn adapter_message_request_is_request() {
        let msg = DebugAdapterMessage::Request {
            seq: 1,
            command: "initialize".into(),
            arguments: None,
        };
        assert!(msg.is_request());
        assert!(!msg.is_response());
        assert!(!msg.is_event());
        assert_eq!(msg.seq(), 1);
        assert_eq!(msg.command_or_event(), "initialize");
    }

    #[test]
    fn adapter_message_response_is_response() {
        let msg = DebugAdapterMessage::Response {
            seq: 2,
            request_seq: 1,
            success: true,
            command: "initialize".into(),
            body: Some(serde_json::json!({"supportsConfigurationDoneRequest": true})),
            message: None,
        };
        assert!(msg.is_response());
        assert!(!msg.is_request());
        assert!(!msg.is_event());
        assert_eq!(msg.seq(), 2);
        assert_eq!(msg.command_or_event(), "initialize");
    }

    #[test]
    fn adapter_message_event_command_or_event() {
        let msg = DebugAdapterMessage::Event {
            seq: 5,
            event: "stopped".into(),
            body: Some(serde_json::json!({"reason": "breakpoint"})),
        };
        assert!(msg.is_event());
        assert_eq!(msg.command_or_event(), "stopped");
    }

    // -- DebugSessionLifecycle -----------------------------------------------

    #[test]
    fn lifecycle_transitions() {
        let mut lc = DebugSessionLifecycle::new("sess-1");
        assert_eq!(lc.current_state(), DebugSessionState::NotStarted);
        assert_eq!(lc.transition_count(), 0);

        lc.record_transition(DebugSessionState::Initializing, 100);
        lc.record_transition(DebugSessionState::Running, 200);
        lc.record_transition(DebugSessionState::Paused, 500);

        assert_eq!(lc.current_state(), DebugSessionState::Paused);
        assert_eq!(lc.transition_count(), 3);
        assert_eq!(lc.history().len(), 3);
    }

    #[test]
    fn lifecycle_time_in_state() {
        let mut lc = DebugSessionLifecycle::new("sess-2");
        lc.record_transition(DebugSessionState::Initializing, 0);
        lc.record_transition(DebugSessionState::Running, 50);
        lc.record_transition(DebugSessionState::Paused, 150);
        lc.record_transition(DebugSessionState::Running, 200);
        lc.record_transition(DebugSessionState::Terminated, 400);

        // Running: [50..150) + [200..400) = 100 + 200 = 300
        assert_eq!(lc.time_in_state(DebugSessionState::Running), Some(300));
        // Initializing: [0..50) = 50
        assert_eq!(lc.time_in_state(DebugSessionState::Initializing), Some(50));
        // Paused: [150..200) = 50
        assert_eq!(lc.time_in_state(DebugSessionState::Paused), Some(50));
        // NotStarted never recorded
        assert_eq!(lc.time_in_state(DebugSessionState::NotStarted), None);
    }

    // -- debug_capabilities_negotiate ----------------------------------------

    #[test]
    fn capabilities_negotiate_merges() {
        let client = serde_json::json!({
            "clientID": "vsedit",
            "adapterID": "test"
        });
        let adapter = serde_json::json!({
            "supportsConfigurationDoneRequest": true,
            "supportsEvaluateForHovers": true,
            "supportsTerminateRequest": true
        });
        let caps = debug_capabilities_negotiate(&client, &adapter);
        assert!(caps.supports_configuration_done);
        assert!(caps.supports_evaluate_for_hovers);
        assert!(caps.supports_terminate_request);
        assert!(!caps.supports_restart);
        assert!(!caps.supports_data_breakpoints);
    }

    // -- DebugAdapterMessageCodec --------------------------------------------

    #[test]
    fn codec_encode_decode_roundtrip() {
        let msg = DebugAdapterMessage::Request {
            seq: 42,
            command: "launch".into(),
            arguments: Some(serde_json::json!({"program": "/bin/test"})),
        };
        let encoded = DebugAdapterMessageCodec::encode(&msg);
        assert!(encoded.starts_with("Content-Length: "));
        assert!(encoded.contains("\r\n\r\n"));

        let decoded = DebugAdapterMessageCodec::decode(&encoded).unwrap();
        assert!(decoded.is_request());
        assert_eq!(decoded.seq(), 42);
        assert_eq!(decoded.command_or_event(), "launch");
    }

    #[test]
    fn codec_decode_invalid_returns_error() {
        // No header at all
        let result = DebugAdapterMessageCodec::decode("just some garbage");
        assert!(result.is_err());

        // Header present but invalid JSON body
        let result = DebugAdapterMessageCodec::decode("Content-Length: 5\r\n\r\nhello");
        assert!(result.is_err());

        // Missing Content-Length prefix
        let result = DebugAdapterMessageCodec::decode("X-Custom: 5\r\n\r\n{}");
        assert!(result.is_err());
    }

    // -- new tests --

    #[test]
    fn extract_command_from_json_request() {
        let json = r#"{"seq":1,"type":"request","command":"initialize"}"#;
        assert_eq!(extract_command_from_json(json), Some("initialize".into()));
    }

    #[test]
    fn extract_command_from_json_event() {
        let json = r#"{"seq":5,"type":"event","event":"stopped"}"#;
        assert_eq!(extract_command_from_json(json), Some("stopped".into()));
    }

    #[test]
    fn extract_seq_from_json_valid() {
        let json = r#"{"seq":42,"type":"request","command":"launch"}"#;
        assert_eq!(extract_seq_from_json(json), Some(42));
    }

    #[test]
    fn extract_seq_from_json_invalid() {
        assert_eq!(extract_seq_from_json("not json"), None);
    }

    #[test]
    fn is_success_response_true() {
        let json = r#"{"seq":1,"type":"response","success":true,"command":"initialize"}"#;
        assert!(is_success_response(json));
    }

    #[test]
    fn is_success_response_false_on_failure() {
        let json = r#"{"seq":1,"type":"response","success":false,"command":"launch"}"#;
        assert!(!is_success_response(json));
    }

    #[test]
    fn breakpoint_manager_toggle_add_remove() {
        let mut mgr = BreakpointManager::new();
        assert!(mgr.toggle("main.rs", 10)); // added
        assert!(mgr.has_breakpoint("main.rs", 10));
        assert_eq!(mgr.total_count(), 1);
        assert!(!mgr.toggle("main.rs", 10)); // removed
        assert!(!mgr.has_breakpoint("main.rs", 10));
        assert_eq!(mgr.total_count(), 0);
    }

    #[test]
    fn breakpoint_manager_multiple_files() {
        let mut mgr = BreakpointManager::new();
        mgr.toggle("a.rs", 1);
        mgr.toggle("a.rs", 5);
        mgr.toggle("b.rs", 10);
        assert_eq!(mgr.total_count(), 3);
        assert_eq!(mgr.file_count(), 2);
        assert_eq!(mgr.get_lines("a.rs"), &[1, 5]);
    }

    #[test]
    fn breakpoint_manager_clear_file() {
        let mut mgr = BreakpointManager::new();
        mgr.toggle("a.rs", 1);
        mgr.toggle("a.rs", 5);
        mgr.clear_file("a.rs");
        assert_eq!(mgr.total_count(), 0);
    }

    #[test]
    fn format_variable_value_with_type() {
        assert_eq!(
            format_variable_value("x", "42", Some("i32")),
            "x: i32 = 42",
        );
    }

    #[test]
    fn format_variable_value_without_type() {
        assert_eq!(format_variable_value("x", "42", None), "x = 42");
    }

    #[test]
    fn truncate_variable_value_long() {
        let result = truncate_variable_value("hello world this is long", 10);
        assert!(result.ends_with('…'));
        assert!(result.chars().count() <= 10);
    }

    #[test]
    fn truncate_variable_value_short_enough() {
        assert_eq!(truncate_variable_value("short", 10), "short");
    }

    #[test]
    fn navigate_stack_up_and_down() {
        assert_eq!(navigate_stack_up(2, 5), 1);
        assert_eq!(navigate_stack_up(0, 5), 0);
        assert_eq!(navigate_stack_down(2, 5), 3);
        assert_eq!(navigate_stack_down(4, 5), 4);
    }

    #[test]
    fn find_frame_by_function_prefix() {
        let frames = vec![
            ParsedStackFrame {
                frame_number: Some(0),
                function_name: "main".into(),
                file_path: Some("main.rs".into()),
                line: Some(10),
            },
            ParsedStackFrame {
                frame_number: Some(1),
                function_name: "std::rt::lang_start".into(),
                file_path: None,
                line: None,
            },
        ];
        assert_eq!(find_frame_by_function(&frames, "std::rt"), Some(1));
        assert_eq!(find_frame_by_function(&frames, "nonexistent"), None);
    }

    #[test]
    fn format_variables_summary_multiline() {
        let vars = vec![
            ("x".into(), "42".into(), Some("i32".into())),
            ("name".into(), "hello".into(), None),
        ];
        let summary = format_variables_summary(&vars);
        assert!(summary.contains("x: i32 = 42"));
        assert!(summary.contains("name = hello"));
        assert_eq!(summary.lines().count(), 2);
    }

    // -- DebugSession additional methods -------------------------------------

    #[test]
    fn session_is_active_and_finished() {
        let mut s = DebugSession::new("s1", "app", "lldb");
        assert!(!s.is_active());
        assert!(!s.is_finished());
        s.initialize().unwrap();
        s.launch(100).unwrap();
        assert!(s.is_active());
        s.pause().unwrap();
        assert!(s.is_active());
        s.terminate().unwrap();
        assert!(s.is_finished());
        assert!(!s.is_active());
    }

    #[test]
    fn session_elapsed_ms() {
        let mut s = DebugSession::new("s2", "app", "lldb");
        assert_eq!(s.elapsed_ms(5000), 0);
        s.initialize().unwrap();
        s.launch(1000).unwrap();
        assert_eq!(s.elapsed_ms(3500), 2500);
    }

    // -- BreakpointManager additional methods --------------------------------

    #[test]
    fn breakpoint_manager_add_and_remove() {
        let mut mgr = BreakpointManager::new();
        assert!(mgr.add("main.rs", 10));
        assert!(!mgr.add("main.rs", 10)); // duplicate
        assert!(mgr.has_breakpoint("main.rs", 10));
        assert!(mgr.remove("main.rs", 10));
        assert!(!mgr.remove("main.rs", 10)); // already removed
        assert!(!mgr.has_breakpoint("main.rs", 10));
    }

    #[test]
    fn breakpoint_manager_files_and_summary() {
        let mut mgr = BreakpointManager::new();
        mgr.add("a.rs", 1);
        mgr.add("a.rs", 5);
        mgr.add("b.rs", 10);
        let files = mgr.files();
        assert!(files.contains(&"a.rs"));
        assert!(files.contains(&"b.rs"));
        let summary = mgr.summary();
        assert_eq!(summary.len(), 2);
    }

    // -- WatchStore additional methods ---------------------------------------

    #[test]
    fn watch_store_has_errors() {
        let mut store = WatchStore::new();
        let id = store.add("x");
        assert!(!store.has_errors());
        store.get_mut(id).unwrap().set_error("undefined");
        assert!(store.has_errors());
        assert_eq!(store.errored_expressions().len(), 1);
    }

    #[test]
    fn watch_store_get_and_clear() {
        let mut store = WatchStore::new();
        let id = store.add("y");
        assert!(store.get(id).is_some());
        assert!(store.get(999).is_none());
        store.clear();
        assert!(store.is_empty());
    }

    // -- DebugSessionState additional methods --------------------------------

    #[test]
    fn session_state_can_initialize() {
        assert!(DebugSessionState::NotStarted.can_initialize());
        assert!(!DebugSessionState::Running.can_initialize());
        assert!(!DebugSessionState::Terminated.can_initialize());
    }

    #[test]
    fn session_state_can_step() {
        assert!(DebugSessionState::Paused.can_step());
        assert!(!DebugSessionState::Running.can_step());
        assert!(!DebugSessionState::NotStarted.can_step());
    }

    // -- Stepping granularity ------------------------------------------------

    #[test]
    fn stepping_granularity_display_and_parse() {
        assert_eq!(SteppingGranularity::Statement.to_string(), "statement");
        assert_eq!(SteppingGranularity::Line.to_string(), "line");
        assert_eq!(SteppingGranularity::Instruction.to_string(), "instruction");

        assert_eq!(
            SteppingGranularity::from_dap_str("statement"),
            Some(SteppingGranularity::Statement)
        );
        assert_eq!(
            SteppingGranularity::from_dap_str("line"),
            Some(SteppingGranularity::Line)
        );
        assert_eq!(
            SteppingGranularity::from_dap_str("instruction"),
            Some(SteppingGranularity::Instruction)
        );
        assert_eq!(SteppingGranularity::from_dap_str("unknown"), None);
    }

    #[test]
    fn stepping_granularity_serde_roundtrip() {
        let g = SteppingGranularity::Instruction;
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(json, r#""instruction""#);
        let parsed: SteppingGranularity = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, g);
    }

    // -- Exception filter store ----------------------------------------------

    #[test]
    fn exception_filter_from_dap_and_store() {
        let body = serde_json::json!({
            "exceptionBreakpointFilters": [
                { "filter": "uncaught", "label": "Uncaught Exceptions", "default": true },
                { "filter": "raised", "label": "All Exceptions", "default": false,
                  "description": "Break on all", "conditionDescription": "module == 'x'" }
            ]
        });
        let mut store = ExceptionFilterStore::new();
        assert!(store.is_empty());
        store.load_from_dap(&body);
        assert_eq!(store.len(), 2);
        assert_eq!(store.enabled_ids(), vec!["uncaught"]);

        // Toggle
        assert_eq!(store.toggle("raised"), Some(true));
        assert_eq!(store.enabled_ids().len(), 2);
        assert_eq!(store.toggle("raised"), Some(false));
        assert_eq!(store.toggle("nonexistent"), None);

        // Condition
        assert!(store.set_condition("uncaught", Some("mymod".into())));
        assert_eq!(
            store.filters().iter().find(|f| f.filter_id == "uncaught").unwrap().condition,
            Some("mymod".to_string())
        );
        assert!(!store.set_condition("nonexistent", None));
    }

    // -- Data breakpoint store -----------------------------------------------

    #[test]
    fn data_breakpoint_store_add_remove() {
        let mut store = DataBreakpointStore::new();
        assert!(store.is_empty());

        let bp = DataBreakpoint::new("var.x", DataBreakpointAccessType::Write);
        store.add(bp.clone());
        assert_eq!(store.len(), 1);

        // Duplicate data_id is ignored
        store.add(bp);
        assert_eq!(store.len(), 1);

        store.add(DataBreakpoint::new("var.y", DataBreakpointAccessType::ReadWrite));
        assert_eq!(store.len(), 2);

        assert!(store.remove("var.x"));
        assert!(!store.remove("var.x"));
        assert_eq!(store.len(), 1);

        let json = store.to_dap_json();
        assert!(json.is_array());
        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["dataId"], "var.y");
    }

    #[test]
    fn data_breakpoint_access_type_display() {
        assert_eq!(DataBreakpointAccessType::Read.to_string(), "read");
        assert_eq!(DataBreakpointAccessType::Write.to_string(), "write");
        assert_eq!(DataBreakpointAccessType::ReadWrite.to_string(), "readWrite");
    }

    // -- Source reference cache -----------------------------------------------

    #[test]
    fn source_reference_cache_register_and_load() {
        let mut cache = SourceReferenceCache::new();
        assert!(cache.is_empty());

        assert!(cache.register(100, "decompiled.cs"));
        assert!(!cache.register(100, "decompiled.cs")); // duplicate
        assert_eq!(cache.len(), 1);

        assert_eq!(cache.unloaded_ids(), vec![100]);

        assert!(cache.set_content(100, "line1\nline2\nline3"));
        assert!(cache.unloaded_ids().is_empty());

        let sr = cache.get(100).unwrap();
        assert!(sr.is_loaded());
        assert_eq!(sr.line_count(), 3);
        assert_eq!(sr.name, "decompiled.cs");

        // set_content for unregistered returns false
        assert!(!cache.set_content(999, "nope"));
    }

    // -- Debug console command parsing ----------------------------------------

    #[test]
    fn parse_debug_console_commands() {
        // Expression evaluation
        assert_eq!(
            parse_debug_console_input("x + 1"),
            DebugConsoleCommand::Evaluate("x + 1".into())
        );

        // Breakpoint toggle
        assert_eq!(
            parse_debug_console_input(".bp main.rs 42"),
            DebugConsoleCommand::ToggleBreakpoint {
                file: "main.rs".into(),
                line: 42,
            }
        );

        // Backtrace
        assert_eq!(
            parse_debug_console_input(".bt"),
            DebugConsoleCommand::Backtrace
        );

        // Vars
        assert_eq!(
            parse_debug_console_input(".vars"),
            DebugConsoleCommand::ListVariables
        );

        // Threads
        assert_eq!(
            parse_debug_console_input(".threads"),
            DebugConsoleCommand::ListThreads
        );

        // Set variable
        assert_eq!(
            parse_debug_console_input(".set count 99"),
            DebugConsoleCommand::SetVariable {
                name: "count".into(),
                value: "99".into(),
            }
        );

        // Unknown dot-command
        match parse_debug_console_input(".foo bar") {
            DebugConsoleCommand::UnknownCommand(s) => assert_eq!(s, ".foo bar"),
            other => panic!("expected UnknownCommand, got {:?}", other),
        }

        // Invalid .bp (missing line)
        match parse_debug_console_input(".bp main.rs") {
            DebugConsoleCommand::UnknownCommand(_) => {}
            other => panic!("expected UnknownCommand, got {:?}", other),
        }
    }

    // -- Conditional breakpoint -----------------------------------------------

    #[test]
    fn conditional_breakpoint_properties() {
        let mut bp = ConditionalBreakpoint::new("main.rs", 10);
        assert!(!bp.is_conditional());
        assert!(!bp.is_logpoint());

        bp.condition = Some("x > 5".into());
        assert!(bp.is_conditional());

        bp.log_message = Some("value of x: {x}".into());
        assert!(bp.is_logpoint());

        let json = bp.to_dap_source_breakpoint();
        assert_eq!(json["line"], 10);
        assert_eq!(json["condition"], "x > 5");
        assert_eq!(json["logMessage"], "value of x: {x}");
    }

    // -- Seq allocator and request builder ------------------------------------

    #[test]
    fn dap_seq_allocator_and_request_builder() {
        let mut alloc = DapSeqAllocator::new();
        assert_eq!(alloc.next(), 1);
        assert_eq!(alloc.next(), 2);

        let msg = alloc.build_request("initialize", Some(serde_json::json!({"a": 1})));
        assert!(msg.is_request());
        assert_eq!(msg.seq(), 3);
        assert_eq!(msg.command_or_event(), "initialize");
    }

    #[test]
    fn build_initialize_args_structure() {
        let args = build_initialize_args("vsedit", "VS Edit", true);
        assert_eq!(args["clientID"], "vsedit");
        assert_eq!(args["clientName"], "VS Edit");
        assert_eq!(args["linesStartAt1"], true);
        assert_eq!(args["supportsVariableType"], true);
    }

    #[test]
    fn build_set_breakpoints_body_structure() {
        let mut bp = ConditionalBreakpoint::new("src/main.rs", 5);
        bp.hit_condition = Some(">= 3".into());
        let body = build_set_breakpoints_body("src/main.rs", &[bp]);
        assert_eq!(body["source"]["path"], "src/main.rs");
        let bps = body["breakpoints"].as_array().unwrap();
        assert_eq!(bps.len(), 1);
        assert_eq!(bps[0]["line"], 5);
        assert_eq!(bps[0]["hitCondition"], ">= 3");
    }

    // -- Instruction breakpoint store -----------------------------------------

    #[test]
    fn instruction_breakpoint_store_operations() {
        let mut store = InstructionBreakpointStore::new();
        assert!(store.is_empty());

        let mut ibp = InstructionBreakpoint::new("0x00401000");
        ibp.offset = Some(4);
        ibp.condition = Some("rax == 0".into());
        store.add(ibp);
        assert_eq!(store.len(), 1);

        // Duplicate same ref + offset is ignored
        let mut ibp2 = InstructionBreakpoint::new("0x00401000");
        ibp2.offset = Some(4);
        store.add(ibp2);
        assert_eq!(store.len(), 1);

        // Different offset is allowed
        store.add(InstructionBreakpoint::new("0x00401000"));
        assert_eq!(store.len(), 2);

        let json = store.to_dap_json();
        assert_eq!(json.as_array().unwrap().len(), 2);
        assert_eq!(json[0]["instructionReference"], "0x00401000");
        assert_eq!(json[0]["offset"], 4);
        assert_eq!(json[0]["condition"], "rax == 0");

        assert!(store.remove("0x00401000"));
        assert!(store.is_empty()); // removes all with that ref
    }

    // -- debug additional tests -------------------------------------------

    #[test]
    fn x_debug_panel_state_new() {
        let p = XDebugPanelState::new(XDebugLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XDebugLayoutRegion::Sidebar);
    }

    #[test]
    fn x_debug_panel_area() {
        let p = XDebugPanelState::new(XDebugLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_debug_panel_toggle() {
        let mut p = XDebugPanelState::new(XDebugLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_debug_panel_resize() {
        let mut p = XDebugPanelState::new(XDebugLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_debug_panel_is_narrow() {
        let mut p = XDebugPanelState::new(XDebugLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_debug_total_visible_area_basic() {
        let panels = vec![
            XDebugPanelState::new(XDebugLayoutRegion::Sidebar, "a"),
            XDebugPanelState::new(XDebugLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_debug_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_debug_total_visible_area_hidden() {
        let mut panels = vec![
            XDebugPanelState::new(XDebugLayoutRegion::Sidebar, "a"),
            XDebugPanelState::new(XDebugLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_debug_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_debug_count_in_region_basic() {
        let panels = vec![
            XDebugPanelState::new(XDebugLayoutRegion::Sidebar, "a"),
            XDebugPanelState::new(XDebugLayoutRegion::Sidebar, "b"),
            XDebugPanelState::new(XDebugLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_debug_count_in_region(&panels, XDebugLayoutRegion::Sidebar), 2);
        assert_eq!(x_debug_count_in_region(&panels, XDebugLayoutRegion::Editor), 1);
        assert_eq!(x_debug_count_in_region(&panels, XDebugLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_debug_widest_panel_basic() {
        let mut panels = vec![
            XDebugPanelState::new(XDebugLayoutRegion::Sidebar, "narrow"),
            XDebugPanelState::new(XDebugLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_debug_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_debug_collapse_region_basic() {
        let mut panels = vec![
            XDebugPanelState::new(XDebugLayoutRegion::Sidebar, "a"),
            XDebugPanelState::new(XDebugLayoutRegion::Sidebar, "b"),
            XDebugPanelState::new(XDebugLayoutRegion::Editor, "c"),
        ];
        x_debug_collapse_region(&mut panels, XDebugLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_debug_layout_constraint_clamp() {
        let lc = XDebugLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_debug_layout_constraint_satisfied() {
        let lc = XDebugLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_debug_widest_panel_empty() {
        let panels: Vec<XDebugPanelState> = vec![];
        assert!(x_debug_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_debug_layout_region_eq() {
        assert_eq!(XDebugLayoutRegion::Sidebar, XDebugLayoutRegion::Sidebar);
        assert_ne!(XDebugLayoutRegion::Sidebar, XDebugLayoutRegion::Panel);
    }

    #[test]
    fn dap_stats_initial() {
        let s = DapMessageStats::new();
        assert_eq!(s.total_messages(), 0);
        assert_eq!(s.error_rate(), 0.0);
        assert_eq!(s.pending_responses(), 0);
    }

    #[test]
    fn dap_stats_record_and_totals() {
        let mut s = DapMessageStats::new();
        s.record_request();
        s.record_response();
        s.record_event();
        assert_eq!(s.total_messages(), 3);
        assert_eq!(s.pending_responses(), 0);
    }

    #[test]
    fn dap_stats_error_rate() {
        let mut s = DapMessageStats::new();
        s.record_request();
        s.record_response();
        s.record_error();
        assert!(s.error_rate() > 0.0);
    }

    #[test]
    fn dap_stats_pending_responses() {
        let mut s = DapMessageStats::new();
        s.record_request();
        s.record_request();
        s.record_response();
        assert_eq!(s.pending_responses(), 1);
    }

    #[test]
    fn dap_stats_reset() {
        let mut s = DapMessageStats::new();
        s.record_request();
        s.record_error();
        s.reset();
        assert_eq!(s.total_messages(), 0);
        assert_eq!(s.errors(), 0);
    }

    #[test]
    fn call_frame_stack_push_pop() {
        let mut stack = CallFrameStack::new();
        stack.push("main", "app.rs", 10);
        assert_eq!(stack.depth(), 1);
        let frame = stack.pop().unwrap();
        assert_eq!(frame.0, "main");
    }

    #[test]
    fn call_frame_stack_top() {
        let mut stack = CallFrameStack::new();
        stack.push("a", "a.rs", 1);
        stack.push("b", "b.rs", 2);
        assert_eq!(stack.top().unwrap().0, "b");
    }

    #[test]
    fn call_frame_stack_contains_source() {
        let mut stack = CallFrameStack::new();
        stack.push("fn1", "lib.rs", 5);
        assert!(stack.contains_source("lib.rs"));
        assert!(!stack.contains_source("main.rs"));
    }

    #[test]
    fn call_frame_stack_clear() {
        let mut stack = CallFrameStack::new();
        stack.push("x", "y.rs", 1);
        stack.clear();
        assert!(stack.is_empty());
    }

    #[test]
    fn call_frame_stack_sources() {
        let mut stack = CallFrameStack::new();
        stack.push("a", "x.rs", 1);
        stack.push("b", "y.rs", 2);
        let sources = stack.sources();
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn watch_evaluator_set_get() {
        let mut eval = DebugWatchEvaluator::new();
        eval.set_variable("x", "42");
        assert_eq!(eval.evaluate("x"), Some("42".into()));
        assert_eq!(eval.evaluate("y"), None);
    }

    #[test]
    fn watch_evaluator_remove() {
        let mut eval = DebugWatchEvaluator::new();
        eval.set_variable("a", "1");
        assert!(eval.remove_variable("a"));
        assert!(!eval.has_variable("a"));
    }

    #[test]
    fn watch_evaluator_names_sorted() {
        let mut eval = DebugWatchEvaluator::new();
        eval.set_variable("z", "1");
        eval.set_variable("a", "2");
        assert_eq!(eval.variable_names(), vec!["a", "z"]);
    }

    #[test]
    fn watch_evaluator_clear() {
        let mut eval = DebugWatchEvaluator::new();
        eval.set_variable("x", "1");
        eval.clear();
        assert_eq!(eval.variable_count(), 0);
    }

    #[test]
    fn call_frame_stack_frame_at() {
        let mut stack = CallFrameStack::new();
        stack.push("a", "a.rs", 1);
        stack.push("b", "b.rs", 2);
        assert_eq!(stack.frame_at(0).unwrap().0, "a");
        assert!(stack.frame_at(5).is_none());
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for debug
    #[test]
    fn xa_debug_ring_new() {
        let rb = super::XaDebugRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_debug_ring_push_len() {
        let mut rb = super::XaDebugRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_debug_ring_wrap() {
        let mut rb = super::XaDebugRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_debug_ring_mean_empty() {
        let rb = super::XaDebugRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_debug_ring_mean_values() {
        let mut rb = super::XaDebugRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_debug_ring_min_max() {
        let mut rb = super::XaDebugRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_debug_ring_iter() {
        let mut rb = super::XaDebugRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_debug_counter_new() {
        let c = super::XaDebugCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_debug_counter_inc() {
        let mut c = super::XaDebugCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_debug_counter_inc_by() {
        let mut c = super::XaDebugCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_debug_counter_reset() {
        let mut c = super::XaDebugCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_debug_counter_clear() {
        let mut c = super::XaDebugCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_debug_counter_default() {
        let c = super::XaDebugCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 26 ----

    #[test]
    fn xc_26_pool_new_empty() {
        let pool: super::Xc26Pool<i32> = super::Xc26Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_26_pool_release_acquire() {
        let mut pool = super::Xc26Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_26_pool_acquire_empty() {
        let mut pool: super::Xc26Pool<i32> = super::Xc26Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_26_pool_full() {
        let mut pool = super::Xc26Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_26_pool_drain() {
        let mut pool = super::Xc26Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_26_pool_stats() {
        let mut pool = super::Xc26Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_26_pool_clear() {
        let mut pool = super::Xc26Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_26_pool_shrink() {
        let mut pool = super::Xc26Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_26_pool_default() {
        let pool: super::Xc26Pool<String> = super::Xc26Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_26_pool_extend() {
        let mut pool = super::Xc26Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_26_pool_retain() {
        let mut pool = super::Xc26Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_26_scheduler_round_robin() {
        let mut sched = super::Xc26Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_26_scheduler_empty() {
        let mut sched = super::Xc26Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_26_scheduler_reset() {
        let mut sched = super::Xc26Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_26_scheduler_add_remove() {
        let mut sched = super::Xc26Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_26_scheduler_targets() {
        let sched = super::Xc26Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_26_hash_empty() {
        assert_eq!(super::xc_26_hash(b""), 5381);
    }

    #[test]
    fn xc_26_hash_data() {
        let h = super::xc_26_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_26_hash(b"hello"), h);
    }

    #[test]
    fn xc_26_reverse_str() {
        assert_eq!(super::xc_26_reverse("abc"), "cba");
        assert_eq!(super::xc_26_reverse(""), "");
    }


    #[test]
    fn xe_3_pipeline_empty() {
        let p = super::Xe3Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_3_pipeline_parse_stage() {
        let p = super::Xe3Pipeline::new()
            .add_parse(super::xe_3_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_3_pipeline_transform_double() {
        let p = super::Xe3Pipeline::new()
            .add_transform(super::xe_3_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_3_pipeline_validate_reverse() {
        let p = super::Xe3Pipeline::new()
            .add_validate(super::xe_3_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_3_pipeline_emit_filter() {
        let p = super::Xe3Pipeline::new()
            .add_emit(super::xe_3_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_3_pipeline_multi_stage() {
        let p = super::Xe3Pipeline::new()
            .add_parse(super::xe_3_pipeline_identity)
            .add_transform(super::xe_3_pipeline_double)
            .add_validate(super::xe_3_pipeline_reverse)
            .add_emit(super::xe_3_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_3_pipeline_error_propagation() {
        let p = super::Xe3Pipeline::new()
            .add_parse(super::xe_3_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe3Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_3_pipeline_compose() {
        let p1 = super::Xe3Pipeline::new()
            .add_parse(super::xe_3_pipeline_identity);
        let p2 = super::Xe3Pipeline::new()
            .add_transform(super::xe_3_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_3_pipeline_error_display() {
        let e = super::Xe3PipelineError {
            stage: super::Xe3Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_3_cache_put_get() {
        let mut c = super::Xe3Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_3_cache_miss() {
        let mut c: super::Xe3Cache<&str, i32> = super::Xe3Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_3_cache_ttl_expiry() {
        let mut c = super::Xe3Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_3_cache_evict() {
        let mut c = super::Xe3Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_3_cache_capacity() {
        let mut c = super::Xe3Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_3_cache_stats() {
        let mut c = super::Xe3Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_3_cache_clear() {
        let mut c = super::Xe3Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #64 --

    #[test]
    fn xf64_trie_insert_search() {
        let mut t = Xf64Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf64_trie_starts_with() {
        let mut t = Xf64Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf64_trie_remove() {
        let mut t = Xf64Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf64_trie_word_count() {
        let mut t = Xf64Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf64_trie_longest_prefix() {
        let mut t = Xf64Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf64_trie_all_words() {
        let mut t = Xf64Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf64_trie_autocomplete() {
        let mut t = Xf64Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf64_trie_empty_search() {
        let t = Xf64Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf64_bloom_add_contains() {
        let mut bf = Xf64BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf64_bloom_probably_absent() {
        let bf = Xf64BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf64_bloom_false_positive_rate() {
        let mut bf = Xf64BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf64_bloom_clear() {
        let mut bf = Xf64BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf64_bloom_union() {
        let mut a = Xf64BloomFilter::xf_new(512, 2);
        let mut b = Xf64BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf64_bloom_intersection_estimate() {
        let mut a = Xf64BloomFilter::xf_new(512, 2);
        let mut b = Xf64BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf64_bloom_union_size_mismatch() {
        let a = Xf64BloomFilter::xf_new(256, 2);
        let b = Xf64BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh25_skip_insert_contains() {
        let mut sl = super::Xh25SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh25_skip_remove() {
        let mut sl = super::Xh25SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh25_skip_len() {
        let mut sl = super::Xh25SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh25_skip_range_query() {
        let mut sl = super::Xh25SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh25_skip_floor_ceiling() {
        let mut sl = super::Xh25SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh25_skip_rank() {
        let mut sl = super::Xh25SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh25_skip_empty() {
        let sl = super::Xh25SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh25_skip_duplicates() {
        let mut sl = super::Xh25SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh25_bitset_set_test() {
        let mut bs = super::Xh25BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh25_bitset_clear_count() {
        let mut bs = super::Xh25BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh25_bitset_and_or_xor() {
        let mut a = super::Xh25BitSet::xh_new(128);
        let mut b = super::Xh25BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh25_bitset_iter_ones() {
        let mut bs = super::Xh25BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh25_bitset_first_last() {
        let mut bs = super::Xh25BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh25_bitset_empty() {
        let bs = super::Xh25BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi25_deque_push_pop_back() {
        let mut dq = super::Xi25Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi25_deque_push_pop_front() {
        let mut dq = super::Xi25Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi25_deque_mixed_ops() {
        let mut dq = super::Xi25Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi25_deque_get_and_split() {
        let mut dq = super::Xi25Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi25_deque_rotate_left() {
        let mut dq = super::Xi25Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi25_deque_rotate_right() {
        let mut dq = super::Xi25Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi25_deque_grow() {
        let mut dq = super::Xi25Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi25_deque_empty() {
        let dq = super::Xi25Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi25_interval_tree_insert_query() {
        let mut tree = super::Xi25IntervalTree::xi_new();
        tree.xi_insert(super::Xi25Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi25Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi25Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi25_interval_tree_overlap() {
        let mut tree = super::Xi25IntervalTree::xi_new();
        tree.xi_insert(super::Xi25Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi25Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi25Interval::xi_new(12, 20));
        let q = super::Xi25Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi25_interval_tree_remove() {
        let mut tree = super::Xi25IntervalTree::xi_new();
        tree.xi_insert(super::Xi25Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi25Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi25_interval_tree_gaps() {
        let mut tree = super::Xi25IntervalTree::xi_new();
        tree.xi_insert(super::Xi25Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi25Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi25Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi25Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi25Interval::xi_new(8, 10));
    }

    #[test]
    fn xi25_interval_tree_merge() {
        let mut tree = super::Xi25IntervalTree::xi_new();
        tree.xi_insert(super::Xi25Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi25Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi25Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi25Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi25Interval::xi_new(10, 15));
    }

    #[test]
    fn xi25_interval_tree_all() {
        let mut tree = super::Xi25IntervalTree::xi_new();
        tree.xi_insert(super::Xi25Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi25Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi25_interval_tree_empty() {
        let tree = super::Xi25IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi25_interval_tree_contains_point() {
        let iv = super::Xi25Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 24) ---

    #[test]
    fn xj_24_uf_make_and_find() {
        let mut uf = super::Xj24UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_24_uf_union_connected() {
        let mut uf = super::Xj24UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_24_uf_component_count() {
        let mut uf = super::Xj24UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_24_uf_component_size() {
        let mut uf = super::Xj24UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_24_uf_largest_component() {
        let mut uf = super::Xj24UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_24_uf_many_elements() {
        let mut uf = super::Xj24UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_24_uf_separate_components() {
        let mut uf = super::Xj24UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_24_uf_path_compression() {
        let mut uf = super::Xj24UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_24_bt_insert_get() {
        let mut bt = super::Xj24BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_24_bt_contains_len() {
        let mut bt = super::Xj24BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_24_bt_replace() {
        let mut bt = super::Xj24BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_24_bt_remove() {
        let mut bt = super::Xj24BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_24_bt_keys_values() {
        let mut bt = super::Xj24BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_24_bt_range() {
        let mut bt = super::Xj24BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_24_bt_min_max() {
        let mut bt = super::Xj24BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_24_bt_many_inserts() {
        let mut bt = super::Xj24BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_24 segment tree tests ---

    #[test]
    fn xk_24_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk24SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_24_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk24SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_24_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk24SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_24_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk24SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_24_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk24SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_24_st_single_element() {
        let data = vec![42];
        let st = super::Xk24SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_24_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk24SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_24_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk24SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_24 disjoint intervals tests ---

    #[test]
    fn xk_24_di_add_and_count() {
        let mut di = super::Xk24DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_24_di_merge_overlap() {
        let mut di = super::Xk24DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_24_di_contains() {
        let mut di = super::Xk24DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_24_di_remove() {
        let mut di = super::Xk24DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_24_di_covered_length() {
        let mut di = super::Xk24DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_24_di_gaps() {
        let mut di = super::Xk24DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_24_di_merge_adjacent() {
        let mut di = super::Xk24DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_24_di_empty() {
        let di = super::Xk24DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_24_rope_new_empty() {
        let rope = super::Xl24Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_24_rope_from_str() {
        let rope = super::Xl24Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_24_rope_insert_at() {
        let mut rope = super::Xl24Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_24_rope_delete_range() {
        let mut rope = super::Xl24Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_24_rope_char_at() {
        let rope = super::Xl24Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_24_rope_split_concat() {
        let rope = super::Xl24Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_24_rope_line_count() {
        let rope = super::Xl24Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_24_rope_line_at() {
        let rope = super::Xl24Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_24_sa_build_and_search() {
        let sa = super::Xl24SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_24_sa_count() {
        let sa = super::Xl24SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_24_sa_longest_repeated() {
        let sa = super::Xl24SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_24_sa_all_positions() {
        let sa = super::Xl24SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_24_sa_len() {
        let sa = super::Xl24SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_24_sa_empty() {
        let sa = super::Xl24SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_24_rope_slice() {
        let rope = super::Xl24Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_24_sa_search_start() {
        let sa = super::Xl24SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
