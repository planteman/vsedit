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

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskDefinition {
    #[serde(rename = "type")]
    pub task_type: String,
    pub attributes: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub name: String,
    pub definition: TaskDefinition,
    pub source: String,
    pub group: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskExecution {
    pub id: String,
    pub task: Task,
    pub is_running: bool,
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
}
