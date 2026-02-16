//! Task runner: build tasks, test tasks, and task execution.

use std::collections::HashMap;
use std::fmt;

// ── Errors ──────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub enum TaskError {
    TaskNotFound(String),
    AlreadyRunning(String),
    ExecutionFailed(String),
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::TaskNotFound(name) => write!(f, "task not found: {name}"),
            TaskError::AlreadyRunning(name) => write!(f, "task already running: {name}"),
            TaskError::ExecutionFailed(msg) => write!(f, "execution failed: {msg}"),
        }
    }
}

// ── Core types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSource {
    Workspace,
    Extension,
    User,
}

impl fmt::Display for TaskSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskSource::Workspace => write!(f, "Workspace"),
            TaskSource::Extension => write!(f, "Extension"),
            TaskSource::User => write!(f, "User"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskGroup {
    Build,
    Test,
    Clean,
    Deploy,
    None,
}

impl fmt::Display for TaskGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskGroup::Build => write!(f, "Build"),
            TaskGroup::Test => write!(f, "Test"),
            TaskGroup::Clean => write!(f, "Clean"),
            TaskGroup::Deploy => write!(f, "Deploy"),
            TaskGroup::None => write!(f, "None"),
        }
    }
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

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}] ({})", self.name, self.group, self.source)
    }
}

pub struct TaskExecution {
    pub task: Task,
    pub running: bool,
    pub exit_code: Option<i32>,
}

impl TaskExecution {
    pub fn is_success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

// ── TaskBuilder ─────────────────────────────────────────────────────────

pub struct TaskBuilder {
    name: String,
    source: TaskSource,
    group: TaskGroup,
    command: String,
    args: Vec<String>,
    definition: TaskDefinition,
    is_background: bool,
    problem_matcher: Option<String>,
}

impl TaskBuilder {
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: TaskSource::Workspace,
            group: TaskGroup::None,
            command: command.into(),
            args: Vec::new(),
            definition: TaskDefinition {
                task_type: "shell".to_string(),
                properties: HashMap::new(),
            },
            is_background: false,
            problem_matcher: None,
        }
    }

    pub fn source(mut self, source: TaskSource) -> Self {
        self.source = source;
        self
    }

    pub fn group(mut self, group: TaskGroup) -> Self {
        self.group = group;
        self
    }

    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn background(mut self, bg: bool) -> Self {
        self.is_background = bg;
        self
    }

    pub fn problem_matcher(mut self, matcher: impl Into<String>) -> Self {
        self.problem_matcher = Some(matcher.into());
        self
    }

    pub fn definition(mut self, def: TaskDefinition) -> Self {
        self.definition = def;
        self
    }

    pub fn build(self) -> Task {
        Task {
            name: self.name,
            source: self.source,
            group: self.group,
            command: self.command,
            args: self.args,
            definition: self.definition,
            is_background: self.is_background,
            problem_matcher: self.problem_matcher,
        }
    }
}

// ── TaskService ─────────────────────────────────────────────────────────

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

    /// Like `run_task` but returns a `Result` with a typed error.
    pub fn try_run_task(&mut self, name: &str) -> Result<usize, TaskError> {
        let task = self
            .tasks
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| TaskError::TaskNotFound(name.to_string()))?
            .clone();
        let idx = self.executions.len();
        self.executions.push(TaskExecution {
            task,
            running: true,
            exit_code: None,
        });
        Ok(idx)
    }

    /// Marks an execution as stopped with the given exit code.
    pub fn stop_task(&mut self, index: usize, exit_code: i32) -> Result<(), TaskError> {
        let exec = self.executions.get_mut(index).ok_or_else(|| {
            TaskError::TaskNotFound(format!("execution index {index}"))
        })?;
        exec.running = false;
        exec.exit_code = Some(exit_code);
        Ok(())
    }

    pub fn get_task_by_name(&self, name: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.name == name)
    }

    pub fn remove_task(&mut self, name: &str) -> Option<Task> {
        let pos = self.tasks.iter().position(|t| t.name == name)?;
        Some(self.tasks.remove(pos))
    }

    pub fn get_executions_for_task(&self, name: &str) -> Vec<&TaskExecution> {
        self.executions.iter().filter(|e| e.task.name == name).collect()
    }

    pub fn clear_completed_executions(&mut self) {
        self.executions.retain(|e| e.running);
    }

    pub fn running_count(&self) -> usize {
        self.executions.iter().filter(|e| e.running).count()
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

// ── Tests ───────────────────────────────────────────────────────────────

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

    #[test]
    fn try_run_task_not_found() {
        let mut svc = TaskService::new();
        let err = svc.try_run_task("missing").unwrap_err();
        assert_eq!(err, TaskError::TaskNotFound("missing".to_string()));
    }

    #[test]
    fn try_run_task_success() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        let idx = svc.try_run_task("build").unwrap();
        assert_eq!(idx, 0);
        assert!(svc.executions[0].running);
    }

    #[test]
    fn stop_task_sets_exit_code() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        let idx = svc.try_run_task("build").unwrap();
        svc.stop_task(idx, 0).unwrap();
        assert!(!svc.executions[0].running);
        assert!(svc.executions[0].is_success());
    }

    #[test]
    fn stop_task_invalid_index() {
        let mut svc = TaskService::new();
        assert!(svc.stop_task(99, 1).is_err());
    }

    #[test]
    fn get_task_by_name() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        assert!(svc.get_task_by_name("build").is_some());
        assert!(svc.get_task_by_name("nope").is_none());
    }

    #[test]
    fn remove_task() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        svc.register_task(make_task("test", TaskGroup::Test));
        let removed = svc.remove_task("build");
        assert!(removed.is_some());
        assert_eq!(svc.task_count(), 1);
        assert!(svc.remove_task("build").is_none());
    }

    #[test]
    fn get_executions_for_task() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        svc.run_task("build");
        svc.run_task("build");
        assert_eq!(svc.get_executions_for_task("build").len(), 2);
        assert_eq!(svc.get_executions_for_task("other").len(), 0);
    }

    #[test]
    fn clear_completed_executions() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        svc.run_task("build");
        svc.run_task("build");
        svc.stop_task(0, 0).unwrap();
        svc.clear_completed_executions();
        assert_eq!(svc.executions.len(), 1);
        assert!(svc.executions[0].running);
    }

    #[test]
    fn running_count() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("a", TaskGroup::Build));
        svc.register_task(make_task("b", TaskGroup::Test));
        svc.run_task("a");
        svc.run_task("b");
        assert_eq!(svc.running_count(), 2);
        svc.stop_task(0, 0).unwrap();
        assert_eq!(svc.running_count(), 1);
    }

    #[test]
    fn execution_is_success() {
        let exec_ok = TaskExecution {
            task: make_task("t", TaskGroup::None),
            running: false,
            exit_code: Some(0),
        };
        let exec_fail = TaskExecution {
            task: make_task("t", TaskGroup::None),
            running: false,
            exit_code: Some(1),
        };
        let exec_none = TaskExecution {
            task: make_task("t", TaskGroup::None),
            running: true,
            exit_code: None,
        };
        assert!(exec_ok.is_success());
        assert!(!exec_fail.is_success());
        assert!(!exec_none.is_success());
    }

    #[test]
    fn task_builder() {
        let task = TaskBuilder::new("deploy", "kubectl")
            .source(TaskSource::User)
            .group(TaskGroup::Deploy)
            .args(vec!["apply".to_string(), "-f".to_string()])
            .background(true)
            .problem_matcher("$kubectl")
            .build();
        assert_eq!(task.name, "deploy");
        assert_eq!(task.command, "kubectl");
        assert_eq!(task.source, TaskSource::User);
        assert_eq!(task.group, TaskGroup::Deploy);
        assert_eq!(task.args.len(), 2);
        assert!(task.is_background);
        assert_eq!(task.problem_matcher, Some("$kubectl".to_string()));
    }

    #[test]
    fn display_impls() {
        assert_eq!(TaskSource::Workspace.to_string(), "Workspace");
        assert_eq!(TaskGroup::Build.to_string(), "Build");
        assert_eq!(TaskGroup::None.to_string(), "None");
        let task = make_task("lint", TaskGroup::Clean);
        assert_eq!(task.to_string(), "lint [Clean] (Workspace)");
    }

    #[test]
    fn task_error_display() {
        let e = TaskError::TaskNotFound("x".into());
        assert_eq!(e.to_string(), "task not found: x");
        let e = TaskError::AlreadyRunning("y".into());
        assert_eq!(e.to_string(), "task already running: y");
        let e = TaskError::ExecutionFailed("boom".into());
        assert_eq!(e.to_string(), "execution failed: boom");
    }
}
