//! Variable substitution engine for task commands.

use std::collections::HashMap;
use std::path::Path;

/// Context for variable substitution, providing values for VS Code-style variables.
#[derive(Debug, Clone, Default)]
pub struct VariableContext {
    pub workspace_folder: String,
    pub file: String,
    pub relative_file: String,
    pub file_basename: String,
    pub file_dirname: String,
    pub file_extname: String,
    pub file_basename_no_extension: String,
    pub line_number: String,
    pub selected_text: String,
    pub env_vars: HashMap<String, String>,
}

impl VariableContext {
    /// Create a context from a workspace folder and optional current file.
    pub fn new(workspace_folder: &str, current_file: Option<&str>) -> Self {
        let mut ctx = Self {
            workspace_folder: workspace_folder.to_string(),
            ..Default::default()
        };
        if let Some(file) = current_file {
            ctx.set_file(file);
        }
        // Import process env vars
        for (key, value) in std::env::vars() {
            ctx.env_vars.insert(key, value);
        }
        ctx
    }

    /// Set the current file and derive related variables.
    pub fn set_file(&mut self, file_path: &str) {
        self.file = file_path.to_string();
        let path = Path::new(file_path);
        self.file_basename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.file_dirname = path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        self.file_extname = path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        self.file_basename_no_extension = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        // Compute relative file
        if !self.workspace_folder.is_empty() && file_path.starts_with(&self.workspace_folder) {
            self.relative_file = file_path
                .strip_prefix(&self.workspace_folder)
                .unwrap_or(file_path)
                .trim_start_matches('/')
                .to_string();
        } else {
            self.relative_file = file_path.to_string();
        }
    }
}

/// Substitute VS Code-style variables in a text string.
///
/// Supported variables:
/// - `${workspaceFolder}`, `${file}`, `${relativeFile}`, `${fileBasename}`
/// - `${fileDirname}`, `${fileExtname}`, `${fileBasenameNoExtension}`
/// - `${lineNumber}`, `${selectedText}`
/// - `${env:VAR_NAME}` — environment variable lookup
pub fn substitute_variables(text: &str, ctx: &VariableContext) -> String {
    let mut result = text.to_string();

    let replacements = [
        ("${workspaceFolder}", &ctx.workspace_folder),
        ("${file}", &ctx.file),
        ("${relativeFile}", &ctx.relative_file),
        ("${fileBasename}", &ctx.file_basename),
        ("${fileDirname}", &ctx.file_dirname),
        ("${fileExtname}", &ctx.file_extname),
        (
            "${fileBasenameNoExtension}",
            &ctx.file_basename_no_extension,
        ),
        ("${lineNumber}", &ctx.line_number),
        ("${selectedText}", &ctx.selected_text),
    ];

    for (var, val) in &replacements {
        result = result.replace(var, val);
    }

    // Handle ${env:VAR_NAME} patterns
    while let Some(start) = result.find("${env:") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 6..start + end];
            let value = ctx
                .env_vars
                .get(var_name)
                .cloned()
                .unwrap_or_default();
            result = format!("{}{}{}", &result[..start], value, &result[start + end + 1..]);
        } else {
            break;
        }
    }

    result
}

/// Substitute variables in a list of strings.
pub fn substitute_variables_vec(items: &[String], ctx: &VariableContext) -> Vec<String> {
    items.iter().map(|s| substitute_variables(s, ctx)).collect()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> VariableContext {
        let mut ctx = VariableContext {
            workspace_folder: "/home/user/project".to_string(),
            env_vars: HashMap::new(),
            ..Default::default()
        };
        ctx.set_file("/home/user/project/src/main.rs");
        ctx.env_vars.insert("HOME".to_string(), "/home/user".to_string());
        ctx.env_vars
            .insert("CARGO_TARGET_DIR".to_string(), "target".to_string());
        ctx
    }

    #[test]
    fn substitute_workspace_folder() {
        let ctx = make_ctx();
        let result = substitute_variables("cd ${workspaceFolder}", &ctx);
        assert_eq!(result, "cd /home/user/project");
    }

    #[test]
    fn substitute_file_variables() {
        let ctx = make_ctx();
        assert_eq!(
            substitute_variables("${file}", &ctx),
            "/home/user/project/src/main.rs"
        );
        assert_eq!(substitute_variables("${relativeFile}", &ctx), "src/main.rs");
        assert_eq!(substitute_variables("${fileBasename}", &ctx), "main.rs");
        assert_eq!(
            substitute_variables("${fileDirname}", &ctx),
            "/home/user/project/src"
        );
        assert_eq!(substitute_variables("${fileExtname}", &ctx), ".rs");
        assert_eq!(
            substitute_variables("${fileBasenameNoExtension}", &ctx),
            "main"
        );
    }

    #[test]
    fn substitute_env_variable() {
        let ctx = make_ctx();
        let result = substitute_variables("home is ${env:HOME}", &ctx);
        assert_eq!(result, "home is /home/user");
    }

    #[test]
    fn substitute_env_variable_missing() {
        let ctx = make_ctx();
        let result = substitute_variables("${env:NONEXISTENT_VAR_12345}", &ctx);
        assert_eq!(result, "");
    }

    #[test]
    fn substitute_multiple_variables() {
        let ctx = make_ctx();
        let result = substitute_variables(
            "rustc ${file} -o ${workspaceFolder}/out/${fileBasenameNoExtension}",
            &ctx,
        );
        assert_eq!(
            result,
            "rustc /home/user/project/src/main.rs -o /home/user/project/out/main"
        );
    }

    #[test]
    fn substitute_no_variables() {
        let ctx = make_ctx();
        let result = substitute_variables("plain text", &ctx);
        assert_eq!(result, "plain text");
    }

    #[test]
    fn substitute_variables_vec_works() {
        let ctx = make_ctx();
        let args = vec![
            "--file".to_string(),
            "${fileBasename}".to_string(),
            "--dir".to_string(),
            "${workspaceFolder}".to_string(),
        ];
        let result = substitute_variables_vec(&args, &ctx);
        assert_eq!(result[1], "main.rs");
        assert_eq!(result[3], "/home/user/project");
    }

    #[test]
    fn variable_context_new_without_file() {
        let ctx = VariableContext::new("/workspace", None);
        assert_eq!(ctx.workspace_folder, "/workspace");
        assert_eq!(ctx.file, "");
    }

    #[test]
    fn variable_context_new_with_file() {
        let ctx = VariableContext::new("/workspace", Some("/workspace/src/lib.rs"));
        assert_eq!(ctx.file_basename, "lib.rs");
        assert_eq!(ctx.relative_file, "src/lib.rs");
    }

    #[test]
    fn substitute_env_multiple() {
        let ctx = make_ctx();
        let result =
            substitute_variables("${env:HOME}/${env:CARGO_TARGET_DIR}", &ctx);
        assert_eq!(result, "/home/user/target");
    }
}
