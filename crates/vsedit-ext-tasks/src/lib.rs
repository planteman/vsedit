//! Ext API: Tasks.
//!
//! RPC bridge between the extension host and the main thread for task providers.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_tasks";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TaskMessage {
    RegisterProvider {
        provider_type: String,
    },
    UnregisterProvider {
        provider_type: String,
    },
    ExecuteTask {
        task: Task,
    },
    TerminateTask {
        execution_id: String,
    },
    FetchTasks {
        filter_type: Option<String>,
    },
}

// ── Error Types ──

/// Errors that can occur during task operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskError {
    /// Task name is empty or blank.
    EmptyName,
    /// Task type is empty or blank.
    EmptyType,
    /// The referenced execution was not found.
    ExecutionNotFound(String),
    /// Provider type is already registered.
    DuplicateProvider(String),
    /// Provider type was not found.
    ProviderNotFound(String),
    /// A validation error with a custom message.
    ValidationError(String),
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::EmptyName => write!(f, "task name must not be empty"),
            TaskError::EmptyType => write!(f, "task type must not be empty"),
            TaskError::ExecutionNotFound(id) => write!(f, "execution not found: {id}"),
            TaskError::DuplicateProvider(t) => write!(f, "provider already registered: {t}"),
            TaskError::ProviderNotFound(t) => write!(f, "provider not found: {t}"),
            TaskError::ValidationError(msg) => write!(f, "validation error: {msg}"),
        }
    }
}

impl std::error::Error for TaskError {}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskDefinition {
    #[serde(rename = "type")]
    pub task_type: String,
    pub attributes: serde_json::Value,
}

impl TaskDefinition {
    /// Create a new task definition with the given type and no attributes.
    pub fn new(task_type: impl Into<String>) -> Self {
        Self {
            task_type: task_type.into(),
            attributes: serde_json::Value::Object(Default::default()),
        }
    }

    /// Create a shell-type task definition with the given command.
    pub fn shell(command: impl Into<String>) -> Self {
        Self {
            task_type: "shell".into(),
            attributes: serde_json::json!({ "command": command.into() }),
        }
    }

    /// Create a process-type task definition.
    pub fn process(program: impl Into<String>, args: &[&str]) -> Self {
        Self {
            task_type: "process".into(),
            attributes: serde_json::json!({
                "program": program.into(),
                "args": args,
            }),
        }
    }

    /// Validate that the definition has a non-empty type.
    pub fn validate(&self) -> Result<(), TaskError> {
        if self.task_type.trim().is_empty() {
            return Err(TaskError::EmptyType);
        }
        Ok(())
    }

    /// Return the command string if this is a shell-type definition.
    pub fn shell_command(&self) -> Option<&str> {
        if self.task_type == "shell" {
            self.attributes.get("command").and_then(|v| v.as_str())
        } else {
            None
        }
    }
}

impl std::fmt::Display for TaskDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.task_type)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub name: String,
    pub definition: TaskDefinition,
    pub source: String,
    pub group: Option<String>,
    pub detail: Option<String>,
}

impl Task {
    /// Validate the task name and definition.
    pub fn validate(&self) -> Result<(), TaskError> {
        if self.name.trim().is_empty() {
            return Err(TaskError::EmptyName);
        }
        self.definition.validate()
    }

    /// Returns true if this task belongs to the given group.
    pub fn is_in_group(&self, group: &str) -> bool {
        self.group.as_deref() == Some(group)
    }

    /// Returns a display label combining name, source, and optional detail.
    pub fn label(&self) -> String {
        match &self.detail {
            Some(d) => format!("{} ({}) - {}", self.name, self.source, d),
            None => format!("{} ({})", self.name, self.source),
        }
    }
}

impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Builder for constructing [`Task`] instances.
#[derive(Debug, Clone)]
pub struct TaskBuilder {
    name: String,
    definition: TaskDefinition,
    source: String,
    group: Option<String>,
    detail: Option<String>,
}

impl TaskBuilder {
    pub fn new(name: impl Into<String>, definition: TaskDefinition) -> Self {
        Self {
            name: name.into(),
            definition,
            source: "workspace".into(),
            group: None,
            detail: None,
        }
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Build and validate the task, returning an error if invalid.
    pub fn build(self) -> Result<Task, TaskError> {
        let task = Task {
            name: self.name,
            definition: self.definition,
            source: self.source,
            group: self.group,
            detail: self.detail,
        };
        task.validate()?;
        Ok(task)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskExecution {
    pub id: String,
    pub task: Task,
    pub is_running: bool,
}

impl TaskExecution {
    /// Returns the elapsed display name for this execution.
    pub fn display_name(&self) -> String {
        let status = if self.is_running { "running" } else { "stopped" };
        format!("[{}] {} ({})", self.id, self.task.name, status)
    }
}

impl std::fmt::Display for TaskExecution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ── Bridge ──

pub struct TaskBridge {
    providers: Vec<String>,
    executions: Vec<TaskExecution>,
    next_id: u64,
}

impl TaskBridge {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            executions: Vec::new(),
            next_id: 1,
        }
    }

    pub fn register_provider(&mut self, task_type: &str) {
        if !self.providers.contains(&task_type.to_string()) {
            self.providers.push(task_type.to_string());
        }
    }

    pub fn unregister_provider(&mut self, task_type: &str) {
        self.providers.retain(|p| p != task_type);
    }

    pub fn execute_task(&mut self, task: Task) -> String {
        let id = format!("exec-{}", self.next_id);
        self.next_id += 1;
        self.executions.push(TaskExecution {
            id: id.clone(),
            task,
            is_running: true,
        });
        id
    }

    pub fn terminate_task(&mut self, execution_id: &str) -> bool {
        if let Some(exec) = self.executions.iter_mut().find(|e| e.id == execution_id) {
            exec.is_running = false;
            true
        } else {
            false
        }
    }

    pub fn running_tasks(&self) -> Vec<&TaskExecution> {
        self.executions.iter().filter(|e| e.is_running).collect()
    }

    /// Return a list of all registered provider types.
    pub fn providers(&self) -> &[String] {
        &self.providers
    }

    /// Returns true if a provider of the given type is registered.
    pub fn has_provider(&self, task_type: &str) -> bool {
        self.providers.iter().any(|p| p == task_type)
    }

    /// Return the total number of executions (running + stopped).
    pub fn execution_count(&self) -> usize {
        self.executions.len()
    }

    /// Look up an execution by id.
    pub fn get_execution(&self, execution_id: &str) -> Option<&TaskExecution> {
        self.executions.iter().find(|e| e.id == execution_id)
    }

    /// Remove all stopped executions, returning how many were removed.
    pub fn gc_stopped(&mut self) -> usize {
        let before = self.executions.len();
        self.executions.retain(|e| e.is_running);
        before - self.executions.len()
    }

    /// Execute a task with validation, returning an error if the task is invalid.
    pub fn execute_validated(&mut self, task: Task) -> Result<String, TaskError> {
        task.validate()?;
        Ok(self.execute_task(task))
    }

    /// Filter executions by task group.
    pub fn executions_in_group(&self, group: &str) -> Vec<&TaskExecution> {
        self.executions
            .iter()
            .filter(|e| e.task.is_in_group(group))
            .collect()
    }

    /// Terminate all running tasks, returning the count terminated.
    pub fn terminate_all(&mut self) -> usize {
        let mut count = 0;
        for exec in &mut self.executions {
            if exec.is_running {
                exec.is_running = false;
                count += 1;
            }
        }
        count
    }

    pub fn handle_message(&mut self, msg: &TaskMessage) -> serde_json::Value {
        match msg {
            TaskMessage::RegisterProvider { provider_type } => {
                self.register_provider(provider_type);
                serde_json::json!({"registered": true})
            }
            TaskMessage::UnregisterProvider { provider_type } => {
                self.unregister_provider(provider_type);
                serde_json::json!({"unregistered": true})
            }
            TaskMessage::ExecuteTask { task } => {
                let id = self.execute_task(task.clone());
                serde_json::json!({"executionId": id})
            }
            TaskMessage::TerminateTask { execution_id } => {
                let ok = self.terminate_task(execution_id);
                serde_json::json!({"terminated": ok})
            }
            TaskMessage::FetchTasks { filter_type } => {
                serde_json::json!({"filter": filter_type, "tasks": []})
            }
        }
    }
}

impl Default for TaskBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TaskBridge(providers={}, executions={}, running={})",
            self.providers.len(),
            self.executions.len(),
            self.running_tasks().len(),
        )
    }
}

/// Initialize the tasks extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_task() -> Task {
        Task {
            name: "build".into(),
            definition: TaskDefinition {
                task_type: "shell".into(),
                attributes: serde_json::json!({"command": "cargo build"}),
            },
            source: "workspace".into(),
            group: Some("build".into()),
            detail: None,
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = TaskMessage::ExecuteTask {
            task: test_task(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: TaskMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn task_serialization() {
        let t = test_task();
        let json = serde_json::to_string(&t).unwrap();
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn bridge_execute_and_terminate() {
        let mut bridge = TaskBridge::new();
        let id = bridge.execute_task(test_task());
        assert_eq!(bridge.running_tasks().len(), 1);
        bridge.terminate_task(&id);
        assert_eq!(bridge.running_tasks().len(), 0);
    }

    #[test]
    fn bridge_register_provider() {
        let mut bridge = TaskBridge::new();
        bridge.register_provider("shell");
        bridge.register_provider("shell");
        assert_eq!(bridge.providers.len(), 1);
    }

    #[test]
    fn bridge_terminate_unknown() {
        let mut bridge = TaskBridge::new();
        assert!(!bridge.terminate_task("nope"));
    }

    #[test]
    fn task_definition_shell_constructor() {
        let def = TaskDefinition::shell("cargo test");
        assert_eq!(def.task_type, "shell");
        assert_eq!(def.shell_command(), Some("cargo test"));
    }

    #[test]
    fn task_definition_process_constructor() {
        let def = TaskDefinition::process("rustc", &["--edition", "2021", "main.rs"]);
        assert_eq!(def.task_type, "process");
        assert_eq!(def.attributes["program"], "rustc");
        assert_eq!(def.attributes["args"][0], "--edition");
    }

    #[test]
    fn task_definition_validate_empty_type() {
        let def = TaskDefinition::new("  ");
        assert_eq!(def.validate(), Err(TaskError::EmptyType));
    }

    #[test]
    fn task_definition_display() {
        let def = TaskDefinition::new("npm");
        assert_eq!(format!("{def}"), "[npm]");
    }

    #[test]
    fn task_validate_empty_name() {
        let task = Task {
            name: "".into(),
            definition: TaskDefinition::new("shell"),
            source: "workspace".into(),
            group: None,
            detail: None,
        };
        assert_eq!(task.validate(), Err(TaskError::EmptyName));
    }

    #[test]
    fn task_label_with_detail() {
        let mut t = test_task();
        t.detail = Some("production".into());
        assert_eq!(t.label(), "build (workspace) - production");
    }

    #[test]
    fn task_is_in_group() {
        let t = test_task();
        assert!(t.is_in_group("build"));
        assert!(!t.is_in_group("test"));
    }

    #[test]
    fn task_builder_success() {
        let task = TaskBuilder::new("lint", TaskDefinition::shell("cargo clippy"))
            .source("user")
            .group("check")
            .detail("runs clippy")
            .build()
            .unwrap();
        assert_eq!(task.name, "lint");
        assert_eq!(task.source, "user");
        assert_eq!(task.group.as_deref(), Some("check"));
        assert_eq!(task.detail.as_deref(), Some("runs clippy"));
    }

    #[test]
    fn task_builder_validation_error() {
        let result = TaskBuilder::new("", TaskDefinition::shell("echo")).build();
        assert!(result.is_err());
    }

    #[test]
    fn bridge_execute_validated_rejects_invalid() {
        let mut bridge = TaskBridge::new();
        let bad_task = Task {
            name: "".into(),
            definition: TaskDefinition::new("shell"),
            source: "ws".into(),
            group: None,
            detail: None,
        };
        assert!(bridge.execute_validated(bad_task).is_err());
        assert_eq!(bridge.execution_count(), 0);
    }

    #[test]
    fn bridge_gc_stopped() {
        let mut bridge = TaskBridge::new();
        let id1 = bridge.execute_task(test_task());
        let _id2 = bridge.execute_task(test_task());
        bridge.terminate_task(&id1);
        assert_eq!(bridge.gc_stopped(), 1);
        assert_eq!(bridge.execution_count(), 1);
    }

    #[test]
    fn bridge_terminate_all() {
        let mut bridge = TaskBridge::new();
        bridge.execute_task(test_task());
        bridge.execute_task(test_task());
        assert_eq!(bridge.terminate_all(), 2);
        assert_eq!(bridge.running_tasks().len(), 0);
    }

    #[test]
    fn bridge_get_execution() {
        let mut bridge = TaskBridge::new();
        let id = bridge.execute_task(test_task());
        assert!(bridge.get_execution(&id).is_some());
        assert!(bridge.get_execution("no-such").is_none());
    }

    #[test]
    fn bridge_has_provider() {
        let mut bridge = TaskBridge::new();
        bridge.register_provider("npm");
        assert!(bridge.has_provider("npm"));
        assert!(!bridge.has_provider("cargo"));
    }

    #[test]
    fn bridge_executions_in_group() {
        let mut bridge = TaskBridge::new();
        bridge.execute_task(test_task()); // group = "build"
        let t2 = TaskBuilder::new("test-all", TaskDefinition::shell("cargo test"))
            .group("test")
            .build()
            .unwrap();
        bridge.execute_task(t2);
        assert_eq!(bridge.executions_in_group("build").len(), 1);
        assert_eq!(bridge.executions_in_group("test").len(), 1);
        assert_eq!(bridge.executions_in_group("deploy").len(), 0);
    }

    #[test]
    fn bridge_display() {
        let bridge = TaskBridge::new();
        let s = format!("{bridge}");
        assert!(s.contains("TaskBridge"));
        assert!(s.contains("providers=0"));
    }

    #[test]
    fn task_error_display() {
        let err = TaskError::EmptyName;
        assert_eq!(format!("{err}"), "task name must not be empty");
        let err2 = TaskError::ExecutionNotFound("x".into());
        assert!(format!("{err2}").contains("x"));
    }

    #[test]
    fn task_execution_display_name() {
        let mut bridge = TaskBridge::new();
        let id = bridge.execute_task(test_task());
        let exec = bridge.get_execution(&id).unwrap();
        let name = exec.display_name();
        assert!(name.contains("running"));
        assert!(name.contains("build"));
    }

    #[test]
    fn handle_message_register_and_unregister() {
        let mut bridge = TaskBridge::new();
        bridge.handle_message(&TaskMessage::RegisterProvider {
            provider_type: "cargo".into(),
        });
        assert!(bridge.has_provider("cargo"));
        bridge.handle_message(&TaskMessage::UnregisterProvider {
            provider_type: "cargo".into(),
        });
        assert!(!bridge.has_provider("cargo"));
    }
}
