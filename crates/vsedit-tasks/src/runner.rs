//! Task runner that spawns and manages child processes.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::definition::{TaskDefinition, TaskType, TasksError};
use crate::execution::{TaskExecution, TaskStatus};
use crate::problem_matcher::{Diagnostic, ProblemMatcher};
use crate::variables::{substitute_variables, substitute_variables_vec, VariableContext};

/// Task runner that executes task definitions as child processes.
pub struct TaskRunner {
    executions: Vec<TaskExecution>,
    matchers: Vec<Box<dyn ProblemMatcher>>,
    diagnostics: Vec<Diagnostic>,
    on_started: Vec<Box<dyn Fn(&str) + Send>>,
    on_ended: Vec<Box<dyn Fn(&str, &TaskStatus) + Send>>,
}

impl TaskRunner {
    pub fn new() -> Self {
        Self {
            executions: Vec::new(),
            matchers: Vec::new(),
            diagnostics: Vec::new(),
            on_started: Vec::new(),
            on_ended: Vec::new(),
        }
    }

    /// Register a problem matcher for output parsing.
    pub fn add_matcher(&mut self, matcher: Box<dyn ProblemMatcher>) {
        self.matchers.push(matcher);
    }

    /// Register a callback for task-started events.
    pub fn on_task_started(&mut self, callback: impl Fn(&str) + Send + 'static) {
        self.on_started.push(Box::new(callback));
    }

    /// Register a callback for task-ended events.
    pub fn on_task_ended(&mut self, callback: impl Fn(&str, &TaskStatus) + Send + 'static) {
        self.on_ended.push(Box::new(callback));
    }

    /// Build the command string from a task definition after variable substitution.
    pub fn build_command(task: &TaskDefinition, ctx: &VariableContext) -> (String, Vec<String>) {
        let command = task
            .command
            .as_ref()
            .map(|c| substitute_variables(c, ctx))
            .unwrap_or_default();
        let args = substitute_variables_vec(&task.args, ctx);
        (command, args)
    }

    /// Run a task synchronously, capturing its output and returning the execution result.
    pub fn run_task(
        &mut self,
        task: &TaskDefinition,
        ctx: &VariableContext,
    ) -> Result<usize, TasksError> {
        let (command, args) = Self::build_command(task, ctx);
        if command.is_empty() {
            return Err(TasksError::ExecutionError(
                "no command specified".to_string(),
            ));
        }

        let mut exec = TaskExecution::new(&task.label, &command);

        // Fire started callbacks
        for cb in &self.on_started {
            cb(&task.label);
        }

        let result = match task.task_type {
            TaskType::Shell => {
                let shell = if cfg!(target_os = "windows") {
                    "cmd"
                } else {
                    "sh"
                };
                let shell_flag = if cfg!(target_os = "windows") {
                    "/C"
                } else {
                    "-c"
                };
                let full_command = if args.is_empty() {
                    command.clone()
                } else {
                    format!("{} {}", command, args.join(" "))
                };

                Command::new(shell)
                    .arg(shell_flag)
                    .arg(&full_command)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
            }
            TaskType::Process | TaskType::Npm => {
                let program = if task.task_type == TaskType::Npm {
                    "npm".to_string()
                } else {
                    command.clone()
                };
                let mut cmd = Command::new(&program);
                if task.task_type == TaskType::Npm {
                    cmd.arg("run").arg(&command);
                }
                cmd.args(&args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                cmd.spawn()
            }
        };

        match result {
            Ok(mut child) => {
                exec.pid = Some(child.id());

                // Read stdout
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            // Run through problem matchers
                            for matcher in &self.matchers {
                                if let Some(diag) = matcher.parse_line(&line) {
                                    self.diagnostics.push(diag);
                                }
                            }
                            exec.push_output(&line);
                        }
                    }
                }

                // Read stderr
                if let Some(stderr) = child.stderr.take() {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            for matcher in &self.matchers {
                                if let Some(diag) = matcher.parse_line(&line) {
                                    self.diagnostics.push(diag);
                                }
                            }
                            exec.push_output(&line);
                        }
                    }
                }

                match child.wait() {
                    Ok(status) => {
                        if status.success() {
                            exec.mark_succeeded();
                        } else {
                            exec.mark_failed(status.code().unwrap_or(-1));
                        }
                    }
                    Err(e) => {
                        exec.mark_failed(-1);
                        exec.push_output(&format!("wait error: {e}"));
                    }
                }
            }
            Err(e) => {
                exec.mark_failed(-1);
                exec.push_output(&format!("spawn error: {e}"));
            }
        }

        // Fire ended callbacks
        let status = exec.status.clone();
        for cb in &self.on_ended {
            cb(&task.label, &status);
        }

        let idx = self.executions.len();
        self.executions.push(exec);
        Ok(idx)
    }

    /// Get all executions.
    pub fn executions(&self) -> &[TaskExecution] {
        &self.executions
    }

    /// Get a specific execution by index.
    pub fn get_execution(&self, index: usize) -> Option<&TaskExecution> {
        self.executions.get(index)
    }

    /// Get all collected diagnostics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Clear all collected diagnostics.
    pub fn clear_diagnostics(&mut self) {
        self.diagnostics.clear();
    }

    /// Number of running tasks.
    pub fn running_count(&self) -> usize {
        self.executions
            .iter()
            .filter(|e| e.status.is_running())
            .count()
    }

    /// Kill a running task by execution index.
    pub fn kill_task(&mut self, index: usize) -> Result<(), TasksError> {
        let exec = self
            .executions
            .get_mut(index)
            .ok_or_else(|| TasksError::TaskNotFound(format!("execution index {index}")))?;
        if exec.status.is_running() {
            exec.mark_cancelled();
            // In a real implementation, we'd send SIGTERM to exec.pid
        }
        Ok(())
    }
}

impl Default for TaskRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo_task(label: &str, msg: &str) -> TaskDefinition {
        TaskDefinition {
            label: label.to_string(),
            task_type: TaskType::Shell,
            command: Some(format!("echo {msg}")),
            args: vec![],
            group: None,
            presentation: Default::default(),
            problem_matcher: vec![],
            is_background: false,
            depends_on: vec![],
            source: "test".to_string(),
        }
    }

    fn default_ctx() -> VariableContext {
        VariableContext::new("/tmp/test-workspace", None)
    }

    #[test]
    fn run_echo_task() {
        let mut runner = TaskRunner::new();
        let task = echo_task("hello", "hello world");
        let ctx = default_ctx();
        let idx = runner.run_task(&task, &ctx).unwrap();
        let exec = runner.get_execution(idx).unwrap();
        assert!(exec.status.is_success());
        assert!(exec.full_output().contains("hello world"));
    }

    #[test]
    fn run_failing_task() {
        let mut runner = TaskRunner::new();
        let task = TaskDefinition {
            label: "fail".to_string(),
            task_type: TaskType::Shell,
            command: Some("exit 1".to_string()),
            args: vec![],
            group: None,
            presentation: Default::default(),
            problem_matcher: vec![],
            is_background: false,
            depends_on: vec![],
            source: "test".to_string(),
        };
        let ctx = default_ctx();
        let idx = runner.run_task(&task, &ctx).unwrap();
        let exec = runner.get_execution(idx).unwrap();
        assert_eq!(exec.status, TaskStatus::Failed(1));
    }

    #[test]
    fn run_task_no_command() {
        let mut runner = TaskRunner::new();
        let task = TaskDefinition {
            label: "empty".to_string(),
            task_type: TaskType::Shell,
            command: None,
            args: vec![],
            group: None,
            presentation: Default::default(),
            problem_matcher: vec![],
            is_background: false,
            depends_on: vec![],
            source: "test".to_string(),
        };
        let ctx = default_ctx();
        let result = runner.run_task(&task, &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn build_command_substitutes_variables() {
        let task = TaskDefinition {
            label: "build".to_string(),
            task_type: TaskType::Shell,
            command: Some("cd ${workspaceFolder} && cargo build".to_string()),
            args: vec!["--manifest-path".to_string(), "${workspaceFolder}/Cargo.toml".to_string()],
            group: None,
            presentation: Default::default(),
            problem_matcher: vec![],
            is_background: false,
            depends_on: vec![],
            source: "test".to_string(),
        };
        let ctx = VariableContext::new("/my/project", None);
        let (cmd, args) = TaskRunner::build_command(&task, &ctx);
        assert!(cmd.contains("/my/project"));
        assert!(args[1].contains("/my/project"));
    }

    #[test]
    fn kill_task_marks_cancelled() {
        let mut runner = TaskRunner::new();
        // Add a fake running execution
        runner.executions.push(TaskExecution::new("watch", "cargo watch"));
        runner.kill_task(0).unwrap();
        assert_eq!(runner.executions[0].status, TaskStatus::Cancelled);
    }

    #[test]
    fn kill_task_invalid_index() {
        let mut runner = TaskRunner::new();
        assert!(runner.kill_task(99).is_err());
    }

    #[test]
    fn runner_diagnostics() {
        let mut runner = TaskRunner::new();
        assert!(runner.diagnostics().is_empty());
        runner.diagnostics.push(crate::problem_matcher::Diagnostic {
            file: "test.rs".to_string(),
            line: 1,
            column: 1,
            severity: crate::problem_matcher::ProblemSeverity::Error,
            message: "test".to_string(),
            code: None,
            source: "test".to_string(),
        });
        assert_eq!(runner.diagnostics().len(), 1);
        runner.clear_diagnostics();
        assert!(runner.diagnostics().is_empty());
    }

    #[test]
    fn runner_running_count() {
        let mut runner = TaskRunner::new();
        runner.executions.push(TaskExecution::new("a", "cmd_a"));
        runner.executions.push(TaskExecution::new("b", "cmd_b"));
        assert_eq!(runner.running_count(), 2);
        runner.executions[0].mark_succeeded();
        assert_eq!(runner.running_count(), 1);
    }

    #[test]
    fn on_task_events_fire() {
        use std::sync::{Arc, Mutex};

        let started = Arc::new(Mutex::new(Vec::new()));
        let ended = Arc::new(Mutex::new(Vec::new()));

        let mut runner = TaskRunner::new();
        let s = started.clone();
        runner.on_task_started(move |label| {
            s.lock().unwrap().push(label.to_string());
        });
        let e = ended.clone();
        runner.on_task_ended(move |label, status| {
            e.lock().unwrap().push(format!("{label}:{status}"));
        });

        let task = echo_task("greet", "hi");
        let ctx = default_ctx();
        runner.run_task(&task, &ctx).unwrap();

        assert_eq!(started.lock().unwrap().len(), 1);
        assert_eq!(started.lock().unwrap()[0], "greet");
        assert_eq!(ended.lock().unwrap().len(), 1);
        assert!(ended.lock().unwrap()[0].contains("greet"));
    }
}
