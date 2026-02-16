//! Auto-detection of tasks from project files (Cargo, NPM, Make).

use std::path::Path;

use crate::definition::{
    ProblemMatcherConfig, TaskDefinition, TaskGroupConfig, TaskGroupKind, TaskPresentation, TaskType,
};

/// Detect available tasks from project files in a workspace directory.
pub fn detect_tasks(workspace_path: &Path) -> Vec<TaskDefinition> {
    let mut tasks = Vec::new();
    detect_cargo_tasks(workspace_path, &mut tasks);
    detect_npm_tasks(workspace_path, &mut tasks);
    detect_make_tasks(workspace_path, &mut tasks);
    tasks
}

fn detect_cargo_tasks(workspace_path: &Path, tasks: &mut Vec<TaskDefinition>) {
    let cargo_toml = workspace_path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return;
    }

    let cargo_tasks = [
        ("cargo build", "build", TaskGroupKind::Build, true),
        ("cargo test", "test", TaskGroupKind::Test, true),
        ("cargo run", "run", TaskGroupKind::None, false),
        ("cargo check", "check", TaskGroupKind::Build, false),
    ];

    for (command, label_suffix, group_kind, is_default) in &cargo_tasks {
        tasks.push(TaskDefinition {
            label: format!("cargo: {label_suffix}"),
            task_type: TaskType::Shell,
            command: Some(command.to_string()),
            args: vec![],
            group: Some(if *is_default {
                TaskGroupConfig::Detailed {
                    kind: *group_kind,
                    is_default: true,
                }
            } else {
                TaskGroupConfig::Simple(*group_kind)
            }),
            presentation: TaskPresentation::default(),
            problem_matcher: vec![ProblemMatcherConfig::Reference("$rustc".to_string())],
            is_background: false,
            depends_on: vec![],
            source: "auto".to_string(),
        });
    }
}

fn detect_npm_tasks(workspace_path: &Path, tasks: &mut Vec<TaskDefinition>) {
    let package_json = workspace_path.join("package.json");
    if !package_json.exists() {
        return;
    }

    let content = match std::fs::read_to_string(&package_json) {
        Ok(c) => c,
        Err(_) => return,
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return,
    };

    if let Some(scripts) = parsed.get("scripts").and_then(|s| s.as_object()) {
        for script_name in scripts.keys() {
            let group = match script_name.as_str() {
                "build" => Some(TaskGroupConfig::Simple(TaskGroupKind::Build)),
                "test" => Some(TaskGroupConfig::Simple(TaskGroupKind::Test)),
                _ => None,
            };

            tasks.push(TaskDefinition {
                label: format!("npm: {script_name}"),
                task_type: TaskType::Npm,
                command: Some(script_name.to_string()),
                args: vec![],
                group,
                presentation: TaskPresentation::default(),
                problem_matcher: vec![],
                is_background: false,
                depends_on: vec![],
                source: "auto".to_string(),
            });
        }
    }
}

fn detect_make_tasks(workspace_path: &Path, tasks: &mut Vec<TaskDefinition>) {
    let makefile = workspace_path.join("Makefile");
    if !makefile.exists() {
        return;
    }

    let content = match std::fs::read_to_string(&makefile) {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines() {
        // Match lines like "target: deps" but not variable assignments or comments
        if let Some(target) = line.split(':').next() {
            let target = target.trim();
            if !target.is_empty()
                && !target.starts_with('#')
                && !target.starts_with('.')
                && !target.starts_with('\t')
                && !target.contains('=')
                && !target.contains('$')
                && target.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                let group = match target {
                    "build" | "all" => Some(TaskGroupConfig::Simple(TaskGroupKind::Build)),
                    "test" | "check" => Some(TaskGroupConfig::Simple(TaskGroupKind::Test)),
                    _ => None,
                };

                tasks.push(TaskDefinition {
                    label: format!("make: {target}"),
                    task_type: TaskType::Shell,
                    command: Some(format!("make {target}")),
                    args: vec![],
                    group,
                    presentation: TaskPresentation::default(),
                    problem_matcher: vec![ProblemMatcherConfig::Reference("$gcc".to_string())],
                    is_background: false,
                    depends_on: vec![],
                    source: "auto".to_string(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_workspace() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "vsedit-tasks-test-{}-{n}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn detect_cargo_tasks_from_workspace() {
        let dir = make_temp_workspace();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let tasks = detect_tasks(&dir);
        let labels: Vec<&str> = tasks.iter().map(|t| t.label.as_str()).collect();
        assert!(labels.contains(&"cargo: build"));
        assert!(labels.contains(&"cargo: test"));
        assert!(labels.contains(&"cargo: run"));
        assert!(labels.contains(&"cargo: check"));

        // Verify build task is default
        let build = tasks.iter().find(|t| t.label == "cargo: build").unwrap();
        assert!(build.group.as_ref().unwrap().is_default());

        cleanup(&dir);
    }

    #[test]
    fn detect_npm_tasks_from_package_json() {
        let dir = make_temp_workspace();
        fs::write(
            dir.join("package.json"),
            r#"{"scripts": {"build": "tsc", "test": "jest", "lint": "eslint ."}}"#,
        )
        .unwrap();

        let tasks = detect_tasks(&dir);
        let labels: Vec<&str> = tasks.iter().map(|t| t.label.as_str()).collect();
        assert!(labels.contains(&"npm: build"));
        assert!(labels.contains(&"npm: test"));
        assert!(labels.contains(&"npm: lint"));

        let build = tasks.iter().find(|t| t.label == "npm: build").unwrap();
        assert_eq!(build.task_type, TaskType::Npm);
        assert_eq!(
            build.group.as_ref().unwrap().kind(),
            TaskGroupKind::Build
        );

        cleanup(&dir);
    }

    #[test]
    fn detect_make_tasks_from_makefile() {
        let dir = make_temp_workspace();
        fs::write(
            dir.join("Makefile"),
            "all: build test\n\nbuild:\n\tgcc -o main main.c\n\ntest:\n\t./run_tests\n\nclean:\n\trm -f main\n",
        )
        .unwrap();

        let tasks = detect_tasks(&dir);
        let labels: Vec<&str> = tasks.iter().map(|t| t.label.as_str()).collect();
        assert!(labels.contains(&"make: all"));
        assert!(labels.contains(&"make: build"));
        assert!(labels.contains(&"make: test"));
        assert!(labels.contains(&"make: clean"));

        cleanup(&dir);
    }

    #[test]
    fn detect_no_tasks_in_empty_dir() {
        let dir = make_temp_workspace();
        let tasks = detect_tasks(&dir);
        assert!(tasks.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn detect_mixed_project() {
        let dir = make_temp_workspace();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"mixed\"\n").unwrap();
        fs::write(
            dir.join("package.json"),
            r#"{"scripts": {"dev": "vite"}}"#,
        )
        .unwrap();

        let tasks = detect_tasks(&dir);
        assert!(tasks.iter().any(|t| t.label.starts_with("cargo:")));
        assert!(tasks.iter().any(|t| t.label.starts_with("npm:")));

        cleanup(&dir);
    }

    #[test]
    fn cargo_tasks_have_rustc_problem_matcher() {
        let dir = make_temp_workspace();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"t\"\n").unwrap();

        let tasks = detect_tasks(&dir);
        let build = tasks.iter().find(|t| t.label == "cargo: build").unwrap();
        assert!(!build.problem_matcher.is_empty());
        match &build.problem_matcher[0] {
            ProblemMatcherConfig::Reference(s) => assert_eq!(s, "$rustc"),
            _ => panic!("expected $rustc reference"),
        }

        cleanup(&dir);
    }

    #[test]
    fn all_auto_tasks_have_auto_source() {
        let dir = make_temp_workspace();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"t\"\n").unwrap();

        let tasks = detect_tasks(&dir);
        for task in &tasks {
            assert_eq!(task.source, "auto");
        }

        cleanup(&dir);
    }
}
