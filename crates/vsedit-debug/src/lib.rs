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
}
