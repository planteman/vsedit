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
// Tests
// ---------------------------------------------------------------------------

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
}
