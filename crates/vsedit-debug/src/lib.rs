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

}
