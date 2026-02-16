//! Task runner: build tasks, test tasks, and task execution.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSource {
    Workspace,
    Extension,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskGroup {
    Build,
    Test,
    Clean,
    Deploy,
    None,
}

#[derive(Debug, Clone)]
pub struct TaskDefinition {
    pub task_type: String,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub source: TaskSource,
    pub group: TaskGroup,
    pub command: String,
    pub args: Vec<String>,
    pub definition: TaskDefinition,
    pub is_background: bool,
    pub problem_matcher: Option<String>,
}

pub struct TaskExecution {
    pub task: Task,
    pub running: bool,
    pub exit_code: Option<i32>,
}

pub struct TaskService {
    pub tasks: Vec<Task>,
    pub executions: Vec<TaskExecution>,
}

impl TaskService {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            executions: Vec::new(),
        }
    }

    pub fn register_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    pub fn get_tasks_by_group(&self, group: TaskGroup) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.group == group).collect()
    }

    /// Starts a task by name. Returns the execution index if found.
    pub fn run_task(&mut self, name: &str) -> Option<usize> {
        let task = self.tasks.iter().find(|t| t.name == name)?.clone();
        let idx = self.executions.len();
        self.executions.push(TaskExecution {
            task,
            running: true,
            exit_code: None,
        });
        Some(idx)
    }

    pub fn get_running(&self) -> Vec<&TaskExecution> {
        self.executions.iter().filter(|e| e.running).collect()
    }

    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

impl Default for TaskService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(name: &str, group: TaskGroup) -> Task {
        Task {
            name: name.to_string(),
            source: TaskSource::Workspace,
            group,
            command: "cargo".to_string(),
            args: vec!["build".to_string()],
            definition: TaskDefinition {
                task_type: "shell".to_string(),
                properties: HashMap::new(),
            },
            is_background: false,
            problem_matcher: None,
        }
    }

    #[test]
    fn register_and_count() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        svc.register_task(make_task("test", TaskGroup::Test));
        assert_eq!(svc.task_count(), 2);
    }

    #[test]
    fn get_tasks_by_group() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        svc.register_task(make_task("lint", TaskGroup::Build));
        svc.register_task(make_task("test", TaskGroup::Test));
        assert_eq!(svc.get_tasks_by_group(TaskGroup::Build).len(), 2);
        assert_eq!(svc.get_tasks_by_group(TaskGroup::Test).len(), 1);
        assert_eq!(svc.get_tasks_by_group(TaskGroup::Clean).len(), 0);
    }

    #[test]
    fn run_task_and_get_running() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        let idx = svc.run_task("build");
        assert_eq!(idx, Some(0));
        assert_eq!(svc.get_running().len(), 1);
        assert!(svc.run_task("nonexistent").is_none());
    }
}
