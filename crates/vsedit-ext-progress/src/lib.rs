//! Ext API: Progress.
//!
//! RPC bridge between the extension host and the main thread for progress reporting.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_progress";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProgressMessage {
    Start {
        handle: u64,
        options: ProgressOptions,
    },
    Report {
        handle: u64,
        increment: Option<f64>,
        message: Option<String>,
    },
    End {
        handle: u64,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ProgressLocation {
    SourceControl,
    Window,
    Notification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProgressOptions {
    pub location: ProgressLocation,
    pub title: Option<String>,
    pub cancellable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProgressState {
    pub handle: u64,
    pub percentage: f64,
    pub message: Option<String>,
    pub is_done: bool,
}

// ── Bridge ──

pub struct ProgressBridge {
    active: Vec<ProgressState>,
}

impl ProgressBridge {
    pub fn new() -> Self {
        Self {
            active: Vec::new(),
        }
    }

    pub fn start(&mut self, handle: u64, options: &ProgressOptions) {
        self.active.push(ProgressState {
            handle,
            percentage: 0.0,
            message: options.title.clone(),
            is_done: false,
        });
    }

    pub fn report(&mut self, handle: u64, increment: Option<f64>, message: Option<String>) {
        if let Some(state) = self.active.iter_mut().find(|s| s.handle == handle) {
            if let Some(inc) = increment {
                state.percentage = (state.percentage + inc).min(100.0);
            }
            if message.is_some() {
                state.message = message;
            }
        }
    }

    pub fn end(&mut self, handle: u64) {
        if let Some(state) = self.active.iter_mut().find(|s| s.handle == handle) {
            state.is_done = true;
            state.percentage = 100.0;
        }
    }

    pub fn active_count(&self) -> usize {
        self.active.iter().filter(|s| !s.is_done).count()
    }

    pub fn get_state(&self, handle: u64) -> Option<&ProgressState> {
        self.active.iter().find(|s| s.handle == handle)
    }

    pub fn handle_message(&mut self, msg: &ProgressMessage) -> serde_json::Value {
        match msg {
            ProgressMessage::Start { handle, options } => {
                self.start(*handle, options);
                serde_json::json!({"started": handle})
            }
            ProgressMessage::Report {
                handle,
                increment,
                message,
            } => {
                self.report(*handle, *increment, message.clone());
                serde_json::json!({"reported": handle})
            }
            ProgressMessage::End { handle } => {
                self.end(*handle);
                serde_json::json!({"ended": handle})
            }
        }
    }
}

impl Default for ProgressBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the progress extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = ProgressMessage::Start {
            handle: 1,
            options: ProgressOptions {
                location: ProgressLocation::Notification,
                title: Some("Loading".into()),
                cancellable: true,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ProgressMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn progress_state_serialization() {
        let state = ProgressState {
            handle: 1,
            percentage: 50.0,
            message: Some("halfway".into()),
            is_done: false,
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: ProgressState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }

    #[test]
    fn bridge_lifecycle() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Window,
            title: Some("work".into()),
            cancellable: false,
        };
        bridge.start(1, &opts);
        assert_eq!(bridge.active_count(), 1);
        bridge.report(1, Some(50.0), None);
        assert_eq!(bridge.get_state(1).unwrap().percentage, 50.0);
        bridge.end(1);
        assert_eq!(bridge.active_count(), 0);
    }

    #[test]
    fn bridge_report_clamps() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Notification,
            title: None,
            cancellable: false,
        };
        bridge.start(1, &opts);
        bridge.report(1, Some(80.0), None);
        bridge.report(1, Some(80.0), None);
        assert_eq!(bridge.get_state(1).unwrap().percentage, 100.0);
    }

    #[test]
    fn bridge_report_unknown_handle() {
        let mut bridge = ProgressBridge::new();
        bridge.report(999, Some(10.0), None);
        assert_eq!(bridge.active_count(), 0);
    }
}
