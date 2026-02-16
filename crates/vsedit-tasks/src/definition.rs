//! Task definition types matching VS Code's tasks.json format.

use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Enums ───────────────────────────────────────────────────────────────

/// Task type matching VS Code task types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    Shell,
    Process,
    Npm,
}

impl Default for TaskType {
    fn default() -> Self {
        Self::Shell
    }
}

/// Task group with optional default flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TaskGroupConfig {
    /// Simple string: "build" or "test"
    Simple(TaskGroupKind),
    /// Object with kind and isDefault
    Detailed {
        kind: TaskGroupKind,
        #[serde(rename = "isDefault", default)]
        is_default: bool,
    },
}

impl TaskGroupConfig {
    pub fn kind(&self) -> TaskGroupKind {
        match self {
            Self::Simple(k) => *k,
            Self::Detailed { kind, .. } => *kind,
        }
    }

    pub fn is_default(&self) -> bool {
        match self {
            Self::Simple(_) => false,
            Self::Detailed { is_default, .. } => *is_default,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskGroupKind {
    Build,
    Test,
    None,
}

impl Default for TaskGroupKind {
    fn default() -> Self {
        Self::None
    }
}

impl std::fmt::Display for TaskGroupKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Build => write!(f, "build"),
            Self::Test => write!(f, "test"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Panel reveal behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RevealKind {
    Always,
    Silent,
    Never,
}

impl Default for RevealKind {
    fn default() -> Self {
        Self::Always
    }
}

/// Terminal panel sharing behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelKind {
    Shared,
    Dedicated,
    New,
}

impl Default for PanelKind {
    fn default() -> Self {
        Self::Shared
    }
}

// ── Presentation ────────────────────────────────────────────────────────

/// Task presentation options controlling terminal behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPresentation {
    #[serde(default)]
    pub reveal: RevealKind,
    #[serde(default = "default_true")]
    pub echo: bool,
    #[serde(default)]
    pub focus: bool,
    #[serde(default)]
    pub panel: PanelKind,
    #[serde(default)]
    pub clear: bool,
    #[serde(default)]
    pub close: bool,
    #[serde(default)]
    pub show_reuse_message: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TaskPresentation {
    fn default() -> Self {
        Self {
            reveal: RevealKind::Always,
            echo: true,
            focus: false,
            panel: PanelKind::Shared,
            clear: false,
            close: false,
            show_reuse_message: false,
        }
    }
}

// ── Problem Matcher Config ──────────────────────────────────────────────

/// Problem matcher pattern from tasks.json.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemMatcherPattern {
    pub regexp: String,
    #[serde(default)]
    pub file: Option<u32>,
    #[serde(default)]
    pub line: Option<u32>,
    #[serde(default)]
    pub column: Option<u32>,
    #[serde(default)]
    pub severity: Option<u32>,
    #[serde(default)]
    pub message: Option<u32>,
    #[serde(default = "default_false")]
    pub r#loop: bool,
}

fn default_false() -> bool {
    false
}

/// Problem matcher configuration from tasks.json.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProblemMatcherConfig {
    /// Reference to a built-in matcher: "$gcc", "$tsc", "$rustc", "$eslint"
    Reference(String),
    /// Inline matcher definition
    Inline {
        #[serde(default)]
        owner: Option<String>,
        #[serde(default)]
        pattern: Option<ProblemMatcherPattern>,
        #[serde(rename = "fileLocation", default)]
        file_location: Option<String>,
        #[serde(default)]
        severity: Option<String>,
    },
}

// ── Task Definition ─────────────────────────────────────────────────────

/// A single task definition as found in tasks.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDefinition {
    pub label: String,
    #[serde(rename = "type", default)]
    pub task_type: TaskType,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub group: Option<TaskGroupConfig>,
    #[serde(default)]
    pub presentation: TaskPresentation,
    #[serde(rename = "problemMatcher", default)]
    pub problem_matcher: Vec<ProblemMatcherConfig>,
    #[serde(rename = "isBackground", default)]
    pub is_background: bool,
    #[serde(rename = "dependsOn", default)]
    pub depends_on: Vec<String>,
    /// Source provenance: "workspace", "auto", etc.
    #[serde(default = "default_workspace")]
    pub source: String,
}

fn default_workspace() -> String {
    "workspace".to_string()
}

impl std::fmt::Display for TaskDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({:?})", self.label, self.task_type)
    }
}

// ── tasks.json file format ──────────────────────────────────────────────

/// Root structure of a tasks.json file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasksJson {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub tasks: Vec<TaskDefinition>,
}

fn default_version() -> String {
    "2.0.0".to_string()
}

/// Error type for tasks operations.
#[derive(Debug, thiserror::Error)]
pub enum TasksError {
    #[error("failed to read tasks.json: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse tasks.json: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("execution error: {0}")]
    ExecutionError(String),
}

/// Parse a tasks.json file and return the task definitions.
pub fn parse_tasks_json(path: &Path) -> Result<Vec<TaskDefinition>, TasksError> {
    let content = std::fs::read_to_string(path)?;
    parse_tasks_json_str(&content)
}

/// Parse tasks.json content from a string.
pub fn parse_tasks_json_str(content: &str) -> Result<Vec<TaskDefinition>, TasksError> {
    let tasks_json: TasksJson = serde_json::from_str(content)?;
    Ok(tasks_json.tasks)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_tasks_json() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "build",
                    "type": "shell",
                    "command": "cargo build"
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].label, "build");
        assert_eq!(tasks[0].task_type, TaskType::Shell);
        assert_eq!(tasks[0].command.as_deref(), Some("cargo build"));
    }

    #[test]
    fn parse_full_tasks_json() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "cargo build",
                    "type": "shell",
                    "command": "cargo",
                    "args": ["build", "--release"],
                    "group": { "kind": "build", "isDefault": true },
                    "presentation": {
                        "reveal": "always",
                        "echo": true,
                        "focus": false,
                        "panel": "shared",
                        "clear": true
                    },
                    "problemMatcher": ["$rustc"],
                    "isBackground": false,
                    "dependsOn": ["clean"]
                },
                {
                    "label": "clean",
                    "type": "shell",
                    "command": "cargo clean"
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(tasks.len(), 2);

        let build = &tasks[0];
        assert_eq!(build.label, "cargo build");
        assert_eq!(build.args, vec!["build", "--release"]);

        let group = build.group.as_ref().unwrap();
        assert_eq!(group.kind(), TaskGroupKind::Build);
        assert!(group.is_default());

        assert!(build.presentation.clear);
        assert_eq!(build.depends_on, vec!["clean"]);
    }

    #[test]
    fn parse_simple_group() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "test",
                    "type": "shell",
                    "command": "cargo test",
                    "group": "test"
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        let group = tasks[0].group.as_ref().unwrap();
        assert_eq!(group.kind(), TaskGroupKind::Test);
        assert!(!group.is_default());
    }

    #[test]
    fn parse_npm_task() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "npm start",
                    "type": "npm",
                    "command": "start",
                    "problemMatcher": []
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(tasks[0].task_type, TaskType::Npm);
    }

    #[test]
    fn parse_process_task_with_args() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "run rustc",
                    "type": "process",
                    "command": "rustc",
                    "args": ["--edition", "2021", "main.rs"]
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(tasks[0].task_type, TaskType::Process);
        assert_eq!(tasks[0].args.len(), 3);
    }

    #[test]
    fn parse_problem_matcher_reference() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [{
                "label": "build",
                "type": "shell",
                "command": "make",
                "problemMatcher": ["$gcc", "$tsc"]
            }]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(tasks[0].problem_matcher.len(), 2);
        match &tasks[0].problem_matcher[0] {
            ProblemMatcherConfig::Reference(s) => assert_eq!(s, "$gcc"),
            _ => panic!("expected reference"),
        }
    }

    #[test]
    fn parse_inline_problem_matcher() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [{
                "label": "build",
                "type": "shell",
                "command": "make",
                "problemMatcher": [{
                    "owner": "custom",
                    "pattern": {
                        "regexp": "^(.+):(\\d+):(\\d+):\\s+(error|warning):\\s+(.*)$",
                        "file": 1,
                        "line": 2,
                        "column": 3,
                        "severity": 4,
                        "message": 5
                    },
                    "fileLocation": "relative",
                    "severity": "error"
                }]
            }]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        match &tasks[0].problem_matcher[0] {
            ProblemMatcherConfig::Inline { owner, pattern, .. } => {
                assert_eq!(owner.as_deref(), Some("custom"));
                assert!(pattern.is_some());
            }
            _ => panic!("expected inline matcher"),
        }
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let result = parse_tasks_json_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn task_definition_display() {
        let task = TaskDefinition {
            label: "build".to_string(),
            task_type: TaskType::Shell,
            command: Some("cargo build".to_string()),
            args: vec![],
            group: None,
            presentation: TaskPresentation::default(),
            problem_matcher: vec![],
            is_background: false,
            depends_on: vec![],
            source: "workspace".to_string(),
        };
        assert_eq!(task.to_string(), "build (Shell)");
    }

    #[test]
    fn presentation_defaults() {
        let p = TaskPresentation::default();
        assert_eq!(p.reveal, RevealKind::Always);
        assert!(p.echo);
        assert!(!p.focus);
        assert_eq!(p.panel, PanelKind::Shared);
        assert!(!p.clear);
        assert!(!p.close);
    }

    #[test]
    fn task_group_kind_display() {
        assert_eq!(TaskGroupKind::Build.to_string(), "build");
        assert_eq!(TaskGroupKind::Test.to_string(), "test");
        assert_eq!(TaskGroupKind::None.to_string(), "none");
    }

    #[test]
    fn parse_tasks_json_file_not_found() {
        let result = parse_tasks_json(Path::new("/nonexistent/tasks.json"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("read tasks.json"));
    }

    #[test]
    fn parse_background_task() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [{
                "label": "watch",
                "type": "shell",
                "command": "cargo watch",
                "isBackground": true
            }]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert!(tasks[0].is_background);
    }
}
