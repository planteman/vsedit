//! Launch configuration parsing (launch.json).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::DapError;

/// A launch configuration entry from launch.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaunchConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub request: String,
    #[serde(default)]
    pub program: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default, rename = "preLaunchTask")]
    pub pre_launch_task: Option<String>,
}

impl LaunchConfig {
    /// Returns `true` if this is a launch request.
    pub fn is_launch(&self) -> bool {
        self.request == "launch"
    }

    /// Returns `true` if this is an attach request.
    pub fn is_attach(&self) -> bool {
        self.request == "attach"
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), DapError> {
        if self.name.trim().is_empty() {
            return Err(DapError::InvalidConfig("name must not be empty".into()));
        }
        if self.type_.trim().is_empty() {
            return Err(DapError::InvalidConfig("type must not be empty".into()));
        }
        if self.request != "launch" && self.request != "attach" {
            return Err(DapError::InvalidConfig(format!(
                "request must be 'launch' or 'attach', got '{}'",
                self.request
            )));
        }
        Ok(())
    }
}

/// The top-level launch.json structure.
#[derive(Debug, Clone, Deserialize)]
struct LaunchJsonFile {
    #[serde(default)]
    configurations: Vec<LaunchConfig>,
}

/// Parse a launch.json file and return the configurations.
pub fn parse_launch_json(content: &str) -> Result<Vec<LaunchConfig>, DapError> {
    // Strip JSON comments (// and /* */) and trailing commas for VS Code compat
    let cleaned = strip_jsonc(content);
    let file: LaunchJsonFile = serde_json::from_str(&cleaned)
        .map_err(|e| DapError::InvalidConfig(format!("failed to parse launch.json: {e}")))?;
    Ok(file.configurations)
}

/// Strip single-line comments, block comments, and trailing commas from JSONC.
fn strip_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    out.push(next);
                    chars.next();
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' => {
                if chars.peek() == Some(&'/') {
                    // Single-line comment: skip to end of line
                    chars.next();
                    for ch in chars.by_ref() {
                        if ch == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                } else if chars.peek() == Some(&'*') {
                    // Block comment: skip to */
                    chars.next();
                    let mut prev = ' ';
                    for ch in chars.by_ref() {
                        if prev == '*' && ch == '/' {
                            break;
                        }
                        if ch == '\n' {
                            out.push('\n');
                        }
                        prev = ch;
                    }
                } else {
                    out.push(c);
                }
            }
            ',' => {
                // Check if this is a trailing comma (followed only by whitespace and ] or })
                let rest: String = chars.clone().collect();
                let trimmed = rest.trim_start();
                if trimmed.starts_with(']') || trimmed.starts_with('}') {
                    // trailing comma — skip it
                } else {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Generate a default launch configuration for common debugger types.
pub fn default_config_for(debugger_type: &str) -> Option<LaunchConfig> {
    match debugger_type {
        "lldb" | "codelldb" => Some(LaunchConfig {
            name: "Debug (LLDB)".into(),
            type_: debugger_type.into(),
            request: "launch".into(),
            program: Some("${workspaceFolder}/target/debug/${workspaceFolderBasename}".into()),
            args: vec![],
            cwd: Some("${workspaceFolder}".into()),
            env: HashMap::new(),
            pre_launch_task: Some("cargo build".into()),
        }),
        "cppdbg" | "cppvsdbg" => Some(LaunchConfig {
            name: "Debug (C/C++)".into(),
            type_: debugger_type.into(),
            request: "launch".into(),
            program: Some("${workspaceFolder}/a.out".into()),
            args: vec![],
            cwd: Some("${workspaceFolder}".into()),
            env: HashMap::new(),
            pre_launch_task: None,
        }),
        "node" | "pwa-node" => Some(LaunchConfig {
            name: "Debug (Node.js)".into(),
            type_: debugger_type.into(),
            request: "launch".into(),
            program: Some("${workspaceFolder}/index.js".into()),
            args: vec![],
            cwd: Some("${workspaceFolder}".into()),
            env: HashMap::new(),
            pre_launch_task: None,
        }),
        "python" | "debugpy" => Some(LaunchConfig {
            name: "Debug (Python)".into(),
            type_: debugger_type.into(),
            request: "launch".into(),
            program: Some("${workspaceFolder}/main.py".into()),
            args: vec![],
            cwd: Some("${workspaceFolder}".into()),
            env: HashMap::new(),
            pre_launch_task: None,
        }),
        "go" | "dlv" => Some(LaunchConfig {
            name: "Debug (Go)".into(),
            type_: debugger_type.into(),
            request: "launch".into(),
            program: Some("${workspaceFolder}".into()),
            args: vec![],
            cwd: Some("${workspaceFolder}".into()),
            env: HashMap::new(),
            pre_launch_task: None,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_launch_json() {
        let json = r#"{
            "version": "0.2.0",
            "configurations": [
                {
                    "name": "Debug",
                    "type": "lldb",
                    "request": "launch",
                    "program": "./target/debug/app",
                    "args": ["--verbose"]
                }
            ]
        }"#;
        let configs = parse_launch_json(json).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "Debug");
        assert_eq!(configs[0].type_, "lldb");
        assert!(configs[0].is_launch());
        assert!(!configs[0].is_attach());
    }

    #[test]
    fn parse_with_comments() {
        let json = r#"{
            // This is a comment
            "version": "0.2.0",
            "configurations": [
                {
                    "name": "Test",
                    "type": "node",
                    "request": "launch",
                    "program": "index.js"
                    /* block comment */
                }
            ]
        }"#;
        let configs = parse_launch_json(json).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "Test");
    }

    #[test]
    fn parse_with_trailing_commas() {
        let json = r#"{
            "configurations": [
                {
                    "name": "Debug",
                    "type": "lldb",
                    "request": "launch",
                    "program": "./app",
                },
            ]
        }"#;
        let configs = parse_launch_json(json).unwrap();
        assert_eq!(configs.len(), 1);
    }

    #[test]
    fn parse_with_env() {
        let json = r#"{
            "configurations": [
                {
                    "name": "Env Test",
                    "type": "node",
                    "request": "launch",
                    "program": "app.js",
                    "env": {"NODE_ENV": "development", "PORT": "3000"}
                }
            ]
        }"#;
        let configs = parse_launch_json(json).unwrap();
        assert_eq!(configs[0].env["NODE_ENV"], "development");
        assert_eq!(configs[0].env["PORT"], "3000");
    }

    #[test]
    fn parse_attach_config() {
        let json = r#"{
            "configurations": [
                {
                    "name": "Attach",
                    "type": "node",
                    "request": "attach"
                }
            ]
        }"#;
        let configs = parse_launch_json(json).unwrap();
        assert!(configs[0].is_attach());
    }

    #[test]
    fn parse_multiple_configs() {
        let json = r#"{
            "configurations": [
                {"name": "A", "type": "lldb", "request": "launch", "program": "a"},
                {"name": "B", "type": "node", "request": "attach"}
            ]
        }"#;
        let configs = parse_launch_json(json).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].name, "A");
        assert_eq!(configs[1].name, "B");
    }

    #[test]
    fn parse_invalid_json_errors() {
        let result = parse_launch_json("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn validate_config() {
        let config = LaunchConfig {
            name: "Test".into(),
            type_: "lldb".into(),
            request: "launch".into(),
            program: Some("./app".into()),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            pre_launch_task: None,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_empty_name_fails() {
        let config = LaunchConfig {
            name: "".into(),
            type_: "lldb".into(),
            request: "launch".into(),
            program: None,
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            pre_launch_task: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_bad_request_fails() {
        let config = LaunchConfig {
            name: "Test".into(),
            type_: "lldb".into(),
            request: "run".into(),
            program: None,
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            pre_launch_task: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn default_config_lldb() {
        let config = default_config_for("lldb").unwrap();
        assert_eq!(config.type_, "lldb");
        assert!(config.is_launch());
        assert!(config.pre_launch_task.is_some());
    }

    #[test]
    fn default_config_node() {
        let config = default_config_for("node").unwrap();
        assert_eq!(config.type_, "node");
    }

    #[test]
    fn default_config_python() {
        let config = default_config_for("python").unwrap();
        assert_eq!(config.type_, "python");
    }

    #[test]
    fn default_config_unknown() {
        assert!(default_config_for("unknown_debugger").is_none());
    }

    #[test]
    fn launch_config_serde_roundtrip() {
        let config = LaunchConfig {
            name: "Test".into(),
            type_: "lldb".into(),
            request: "launch".into(),
            program: Some("./app".into()),
            args: vec!["--flag".into()],
            cwd: Some("/tmp".into()),
            env: HashMap::from([("KEY".into(), "VAL".into())]),
            pre_launch_task: Some("build".into()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: LaunchConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, back);
    }

    #[test]
    fn strip_jsonc_preserves_strings_with_slashes() {
        let input = r#"{"key": "http://example.com"}"#;
        let result = strip_jsonc(input);
        assert!(result.contains("http://example.com"));
    }
}
