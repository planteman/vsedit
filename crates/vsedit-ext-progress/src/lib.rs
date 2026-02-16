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

// ── Error Types ──

/// Errors that can occur during progress operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressError {
    /// The specified handle does not correspond to any active progress.
    HandleNotFound(u64),
    /// The increment value is invalid (negative or NaN).
    InvalidIncrement(String),
    /// A progress with this handle already exists.
    DuplicateHandle(u64),
    /// The title exceeds the maximum allowed length.
    TitleTooLong { max: usize, actual: usize },
    /// The percentage is out of the valid 0..=100 range.
    PercentageOutOfRange(f64),
}

impl std::fmt::Display for ProgressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgressError::HandleNotFound(h) => write!(f, "progress handle {h} not found"),
            ProgressError::InvalidIncrement(reason) => {
                write!(f, "invalid increment: {reason}")
            }
            ProgressError::DuplicateHandle(h) => {
                write!(f, "progress handle {h} already exists")
            }
            ProgressError::TitleTooLong { max, actual } => {
                write!(f, "title length {actual} exceeds maximum {max}")
            }
            ProgressError::PercentageOutOfRange(v) => {
                write!(f, "percentage {v} is outside 0..=100")
            }
        }
    }
}

impl std::error::Error for ProgressError {}

// ── Display implementations ──

impl std::fmt::Display for ProgressLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgressLocation::SourceControl => write!(f, "Source Control"),
            ProgressLocation::Window => write!(f, "Window"),
            ProgressLocation::Notification => write!(f, "Notification"),
        }
    }
}

impl std::fmt::Display for ProgressState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.is_done { "done" } else { "active" };
        let msg = self.message.as_deref().unwrap_or("(no message)");
        write!(
            f,
            "[handle={}] {:.1}% — {} [{}]",
            self.handle, self.percentage, msg, status
        )
    }
}

impl std::fmt::Display for ProgressOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let title = self.title.as_deref().unwrap_or("(untitled)");
        let cancel = if self.cancellable { "cancellable" } else { "non-cancellable" };
        write!(f, "{} @ {} [{}]", title, self.location, cancel)
    }
}

// ── ProgressOptions builder ──

/// Maximum allowed title length for validation.
const MAX_TITLE_LEN: usize = 256;

/// Builder for constructing [`ProgressOptions`] with validation.
#[derive(Debug, Clone)]
pub struct ProgressOptionsBuilder {
    location: ProgressLocation,
    title: Option<String>,
    cancellable: bool,
}

impl ProgressOptionsBuilder {
    pub fn new(location: ProgressLocation) -> Self {
        Self {
            location,
            title: None,
            cancellable: false,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn cancellable(mut self, cancellable: bool) -> Self {
        self.cancellable = cancellable;
        self
    }

    /// Build the [`ProgressOptions`], validating constraints.
    pub fn build(self) -> Result<ProgressOptions, ProgressError> {
        if let Some(ref t) = self.title {
            if t.len() > MAX_TITLE_LEN {
                return Err(ProgressError::TitleTooLong {
                    max: MAX_TITLE_LEN,
                    actual: t.len(),
                });
            }
        }
        Ok(ProgressOptions {
            location: self.location,
            title: self.title,
            cancellable: self.cancellable,
        })
    }
}

// ── ProgressState helpers ──

impl ProgressState {
    /// Returns the remaining percentage until completion.
    pub fn remaining(&self) -> f64 {
        (100.0 - self.percentage).max(0.0)
    }

    /// Returns `true` if this progress has reached 100% or been marked done.
    pub fn is_complete(&self) -> bool {
        self.is_done || self.percentage >= 100.0
    }
}

// ── Extended ProgressBridge methods ──

impl ProgressBridge {
    /// Start a progress with duplicate-handle checking.
    pub fn try_start(
        &mut self,
        handle: u64,
        options: &ProgressOptions,
    ) -> Result<(), ProgressError> {
        if self.active.iter().any(|s| s.handle == handle) {
            return Err(ProgressError::DuplicateHandle(handle));
        }
        self.start(handle, options);
        Ok(())
    }

    /// Report progress with validation on the increment value.
    pub fn try_report(
        &mut self,
        handle: u64,
        increment: Option<f64>,
        message: Option<String>,
    ) -> Result<(), ProgressError> {
        if let Some(inc) = increment {
            if inc.is_nan() {
                return Err(ProgressError::InvalidIncrement("NaN".into()));
            }
            if inc < 0.0 {
                return Err(ProgressError::InvalidIncrement(format!(
                    "negative value {inc}"
                )));
            }
        }
        if !self.active.iter().any(|s| s.handle == handle) {
            return Err(ProgressError::HandleNotFound(handle));
        }
        self.report(handle, increment, message);
        Ok(())
    }

    /// End a progress, returning an error if the handle is unknown.
    pub fn try_end(&mut self, handle: u64) -> Result<(), ProgressError> {
        if !self.active.iter().any(|s| s.handle == handle) {
            return Err(ProgressError::HandleNotFound(handle));
        }
        self.end(handle);
        Ok(())
    }

    /// Remove all completed progress entries and return the count removed.
    pub fn gc_completed(&mut self) -> usize {
        let before = self.active.len();
        self.active.retain(|s| !s.is_done);
        before - self.active.len()
    }

    /// Returns an iterator over all active (non-done) progress states.
    pub fn active_states(&self) -> impl Iterator<Item = &ProgressState> {
        self.active.iter().filter(|s| !s.is_done)
    }

    /// Returns the total number of tracked progress entries (including done).
    pub fn total_count(&self) -> usize {
        self.active.len()
    }

    /// Compute the average percentage across all active (non-done) entries.
    pub fn average_progress(&self) -> Option<f64> {
        let active: Vec<_> = self.active.iter().filter(|s| !s.is_done).collect();
        if active.is_empty() {
            return None;
        }
        let sum: f64 = active.iter().map(|s| s.percentage).sum();
        Some(sum / active.len() as f64)
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

    // ── Additional tests ──

    #[test]
    fn error_display_messages() {
        assert_eq!(
            ProgressError::HandleNotFound(42).to_string(),
            "progress handle 42 not found"
        );
        assert_eq!(
            ProgressError::DuplicateHandle(7).to_string(),
            "progress handle 7 already exists"
        );
        assert_eq!(
            ProgressError::InvalidIncrement("NaN".into()).to_string(),
            "invalid increment: NaN"
        );
        let err = ProgressError::TitleTooLong { max: 256, actual: 300 };
        assert_eq!(err.to_string(), "title length 300 exceeds maximum 256");
        assert_eq!(
            ProgressError::PercentageOutOfRange(120.0).to_string(),
            "percentage 120 is outside 0..=100"
        );
    }

    #[test]
    fn progress_location_display() {
        assert_eq!(ProgressLocation::SourceControl.to_string(), "Source Control");
        assert_eq!(ProgressLocation::Window.to_string(), "Window");
        assert_eq!(ProgressLocation::Notification.to_string(), "Notification");
    }

    #[test]
    fn progress_state_display() {
        let state = ProgressState {
            handle: 5,
            percentage: 33.3,
            message: Some("compiling".into()),
            is_done: false,
        };
        let display = state.to_string();
        assert!(display.contains("33.3%"));
        assert!(display.contains("compiling"));
        assert!(display.contains("active"));
    }

    #[test]
    fn progress_options_display() {
        let opts = ProgressOptions {
            location: ProgressLocation::Notification,
            title: Some("Installing".into()),
            cancellable: true,
        };
        let display = opts.to_string();
        assert!(display.contains("Installing"));
        assert!(display.contains("cancellable"));
    }

    #[test]
    fn builder_basic() {
        let opts = ProgressOptionsBuilder::new(ProgressLocation::Window)
            .title("Build")
            .cancellable(true)
            .build()
            .unwrap();
        assert_eq!(opts.location, ProgressLocation::Window);
        assert_eq!(opts.title.as_deref(), Some("Build"));
        assert!(opts.cancellable);
    }

    #[test]
    fn builder_title_too_long() {
        let long_title = "x".repeat(300);
        let result = ProgressOptionsBuilder::new(ProgressLocation::Notification)
            .title(long_title)
            .build();
        assert!(matches!(
            result,
            Err(ProgressError::TitleTooLong { max: 256, actual: 300 })
        ));
    }

    #[test]
    fn builder_no_title() {
        let opts = ProgressOptionsBuilder::new(ProgressLocation::SourceControl)
            .build()
            .unwrap();
        assert!(opts.title.is_none());
        assert!(!opts.cancellable);
    }

    #[test]
    fn try_start_duplicate() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Window,
            title: None,
            cancellable: false,
        };
        bridge.try_start(1, &opts).unwrap();
        let err = bridge.try_start(1, &opts).unwrap_err();
        assert_eq!(err, ProgressError::DuplicateHandle(1));
    }

    #[test]
    fn try_report_validation() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Window,
            title: None,
            cancellable: false,
        };
        bridge.try_start(1, &opts).unwrap();

        // NaN increment
        let err = bridge.try_report(1, Some(f64::NAN), None).unwrap_err();
        assert!(matches!(err, ProgressError::InvalidIncrement(_)));

        // Negative increment
        let err = bridge.try_report(1, Some(-5.0), None).unwrap_err();
        assert!(matches!(err, ProgressError::InvalidIncrement(_)));

        // Unknown handle
        let err = bridge.try_report(99, Some(10.0), None).unwrap_err();
        assert_eq!(err, ProgressError::HandleNotFound(99));

        // Valid report
        bridge.try_report(1, Some(25.0), Some("quarter".into())).unwrap();
        assert_eq!(bridge.get_state(1).unwrap().percentage, 25.0);
    }

    #[test]
    fn try_end_unknown() {
        let mut bridge = ProgressBridge::new();
        let err = bridge.try_end(42).unwrap_err();
        assert_eq!(err, ProgressError::HandleNotFound(42));
    }

    #[test]
    fn gc_completed_removes_done() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Notification,
            title: None,
            cancellable: false,
        };
        bridge.start(1, &opts);
        bridge.start(2, &opts);
        bridge.start(3, &opts);
        bridge.end(1);
        bridge.end(3);

        assert_eq!(bridge.total_count(), 3);
        let removed = bridge.gc_completed();
        assert_eq!(removed, 2);
        assert_eq!(bridge.total_count(), 1);
        assert!(bridge.get_state(2).is_some());
    }

    #[test]
    fn average_progress() {
        let mut bridge = ProgressBridge::new();
        assert_eq!(bridge.average_progress(), None);

        let opts = ProgressOptions {
            location: ProgressLocation::Window,
            title: None,
            cancellable: false,
        };
        bridge.start(1, &opts);
        bridge.start(2, &opts);
        bridge.report(1, Some(40.0), None);
        bridge.report(2, Some(60.0), None);

        let avg = bridge.average_progress().unwrap();
        assert!((avg - 50.0).abs() < f64::EPSILON);

        // Done entries are excluded from the average
        bridge.end(2);
        let avg = bridge.average_progress().unwrap();
        assert!((avg - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_state_remaining_and_complete() {
        let mut state = ProgressState {
            handle: 1,
            percentage: 75.0,
            message: None,
            is_done: false,
        };
        assert!((state.remaining() - 25.0).abs() < f64::EPSILON);
        assert!(!state.is_complete());

        state.percentage = 100.0;
        assert!(state.is_complete());
        assert!((state.remaining()).abs() < f64::EPSILON);
    }

    #[test]
    fn handle_message_roundtrip() {
        let mut bridge = ProgressBridge::new();
        let start_msg = ProgressMessage::Start {
            handle: 10,
            options: ProgressOptions {
                location: ProgressLocation::Notification,
                title: Some("indexing".into()),
                cancellable: false,
            },
        };
        let result = bridge.handle_message(&start_msg);
        assert_eq!(result["started"], 10);

        let report_msg = ProgressMessage::Report {
            handle: 10,
            increment: Some(50.0),
            message: Some("halfway".into()),
        };
        let result = bridge.handle_message(&report_msg);
        assert_eq!(result["reported"], 10);
        assert_eq!(bridge.get_state(10).unwrap().percentage, 50.0);

        let end_msg = ProgressMessage::End { handle: 10 };
        let result = bridge.handle_message(&end_msg);
        assert_eq!(result["ended"], 10);
        assert!(bridge.get_state(10).unwrap().is_done);
    }

    #[test]
    fn error_is_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(ProgressError::HandleNotFound(1));
        assert!(err.to_string().contains("handle 1"));
    }
}
