//! Task execution types and status tracking.

use std::time::Instant;

/// Status of a task execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Succeeded,
    Failed(i32),
    Cancelled,
}

impl TaskStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Succeeded)
    }

    pub fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Succeeded => Some(0),
            Self::Failed(code) => Some(*code),
            _ => None,
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Succeeded => write!(f, "succeeded"),
            Self::Failed(code) => write!(f, "failed (exit code {code})"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Represents an active or completed task execution.
pub struct TaskExecution {
    pub task_label: String,
    pub command: String,
    pub status: TaskStatus,
    pub start_time: Instant,
    pub terminal_id: Option<u32>,
    pub output: Vec<String>,
    pub pid: Option<u32>,
}

impl TaskExecution {
    /// Create a new running task execution.
    pub fn new(label: &str, command: &str) -> Self {
        Self {
            task_label: label.to_string(),
            command: command.to_string(),
            status: TaskStatus::Running,
            start_time: Instant::now(),
            terminal_id: None,
            output: Vec::new(),
            pid: None,
        }
    }

    /// Elapsed time since the task started.
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Append a line to the captured output.
    pub fn push_output(&mut self, line: &str) {
        self.output.push(line.to_string());
    }

    /// Mark the task as succeeded.
    pub fn mark_succeeded(&mut self) {
        self.status = TaskStatus::Succeeded;
    }

    /// Mark the task as failed with an exit code.
    pub fn mark_failed(&mut self, exit_code: i32) {
        self.status = TaskStatus::Failed(exit_code);
    }

    /// Mark the task as cancelled.
    pub fn mark_cancelled(&mut self) {
        self.status = TaskStatus::Cancelled;
    }

    /// Get all captured output as a single string.
    pub fn full_output(&self) -> String {
        self.output.join("\n")
    }
}

impl std::fmt::Display for TaskExecution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {} ({})", self.task_label, self.command, self.status)
    }
}

/// Callback type aliases for task events.
pub type OnTaskStarted = Box<dyn Fn(&str) + Send>;
pub type OnTaskEnded = Box<dyn Fn(&str, &TaskStatus) + Send>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_is_running() {
        assert!(TaskStatus::Running.is_running());
        assert!(!TaskStatus::Succeeded.is_running());
        assert!(!TaskStatus::Failed(1).is_running());
        assert!(!TaskStatus::Cancelled.is_running());
    }

    #[test]
    fn task_status_is_success() {
        assert!(TaskStatus::Succeeded.is_success());
        assert!(!TaskStatus::Running.is_success());
        assert!(!TaskStatus::Failed(1).is_success());
    }

    #[test]
    fn task_status_exit_code() {
        assert_eq!(TaskStatus::Succeeded.exit_code(), Some(0));
        assert_eq!(TaskStatus::Failed(42).exit_code(), Some(42));
        assert_eq!(TaskStatus::Running.exit_code(), None);
        assert_eq!(TaskStatus::Cancelled.exit_code(), None);
    }

    #[test]
    fn task_status_display() {
        assert_eq!(TaskStatus::Running.to_string(), "running");
        assert_eq!(TaskStatus::Succeeded.to_string(), "succeeded");
        assert_eq!(TaskStatus::Failed(1).to_string(), "failed (exit code 1)");
        assert_eq!(TaskStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn task_execution_new() {
        let exec = TaskExecution::new("build", "cargo build");
        assert_eq!(exec.task_label, "build");
        assert_eq!(exec.command, "cargo build");
        assert!(exec.status.is_running());
        assert!(exec.output.is_empty());
        assert!(exec.pid.is_none());
        assert!(exec.terminal_id.is_none());
    }

    #[test]
    fn task_execution_push_output() {
        let mut exec = TaskExecution::new("test", "cargo test");
        exec.push_output("running 3 tests");
        exec.push_output("test result: ok. 3 passed");
        assert_eq!(exec.output.len(), 2);
        assert!(exec.full_output().contains("running 3 tests"));
        assert!(exec.full_output().contains("test result"));
    }

    #[test]
    fn task_execution_mark_succeeded() {
        let mut exec = TaskExecution::new("build", "cargo build");
        exec.mark_succeeded();
        assert!(exec.status.is_success());
    }

    #[test]
    fn task_execution_mark_failed() {
        let mut exec = TaskExecution::new("build", "cargo build");
        exec.mark_failed(1);
        assert_eq!(exec.status, TaskStatus::Failed(1));
    }

    #[test]
    fn task_execution_mark_cancelled() {
        let mut exec = TaskExecution::new("watch", "cargo watch");
        exec.mark_cancelled();
        assert_eq!(exec.status, TaskStatus::Cancelled);
    }

    #[test]
    fn task_execution_elapsed() {
        let exec = TaskExecution::new("build", "cargo build");
        // Just verify it doesn't panic and returns a non-negative duration
        assert!(exec.elapsed().as_secs() < 60);
    }

    #[test]
    fn task_execution_display() {
        let exec = TaskExecution::new("build", "cargo build");
        let display = exec.to_string();
        assert!(display.contains("build"));
        assert!(display.contains("cargo build"));
        assert!(display.contains("running"));
    }
}
