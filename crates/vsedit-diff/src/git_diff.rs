//! Git diff integration — diff files against git HEAD/index.

use std::path::Path;
use std::process::Command;

use crate::diff_result::{compute_diff, DiffResult};

/// Error type for git operations.
#[derive(Debug)]
pub enum GitDiffError {
    /// Git command failed.
    CommandFailed(String),
    /// File not found or not tracked.
    FileNotFound(String),
    /// Failed to parse git output.
    ParseError(String),
}

impl std::fmt::Display for GitDiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitDiffError::CommandFailed(msg) => write!(f, "git command failed: {msg}"),
            GitDiffError::FileNotFound(path) => write!(f, "file not found: {path}"),
            GitDiffError::ParseError(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for GitDiffError {}

/// Get the content of a file at a specific git ref.
pub fn git_show(path: &Path, git_ref: &str) -> Result<String, GitDiffError> {
    let repo_root = find_repo_root(path)?;
    let relative = path
        .strip_prefix(&repo_root)
        .map_err(|e| GitDiffError::ParseError(e.to_string()))?;

    let output = Command::new("git")
        .args(["show", &format!("{}:{}", git_ref, relative.display())])
        .current_dir(&repo_root)
        .output()
        .map_err(|e| GitDiffError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitDiffError::CommandFailed(stderr.to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Diff a file against git HEAD.
pub fn git_diff_file(path: &Path) -> Result<DiffResult, GitDiffError> {
    let head_content = git_show(path, "HEAD")?;
    let current_content =
        std::fs::read_to_string(path).map_err(|e| GitDiffError::FileNotFound(e.to_string()))?;
    Ok(compute_diff(&head_content, &current_content))
}

/// Diff staged changes for a file.
pub fn git_diff_staged(path: &Path) -> Result<DiffResult, GitDiffError> {
    let head_content = git_show(path, "HEAD")?;
    let staged_content = git_show_staged(path)?;
    Ok(compute_diff(&head_content, &staged_content))
}

/// Get the staged (index) content of a file.
fn git_show_staged(path: &Path) -> Result<String, GitDiffError> {
    let repo_root = find_repo_root(path)?;
    let relative = path
        .strip_prefix(&repo_root)
        .map_err(|e| GitDiffError::ParseError(e.to_string()))?;

    let output = Command::new("git")
        .args(["show", &format!(":{}", relative.display())])
        .current_dir(&repo_root)
        .output()
        .map_err(|e| GitDiffError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitDiffError::CommandFailed(stderr.to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Find the git repository root from a file path.
fn find_repo_root(path: &Path) -> Result<std::path::PathBuf, GitDiffError> {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .ok_or_else(|| GitDiffError::FileNotFound(path.display().to_string()))?
            .to_path_buf()
    };

    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&dir)
        .output()
        .map_err(|e| GitDiffError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(GitDiffError::CommandFailed(
            "not a git repository".to_string(),
        ));
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(std::path::PathBuf::from(root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn git_diff_error_display() {
        let e = GitDiffError::CommandFailed("test".into());
        assert!(e.to_string().contains("test"));
        let e = GitDiffError::FileNotFound("foo.rs".into());
        assert!(e.to_string().contains("foo.rs"));
        let e = GitDiffError::ParseError("bad".into());
        assert!(e.to_string().contains("bad"));
    }

    #[test]
    fn git_show_nonexistent_file() {
        let result = git_show(&PathBuf::from("/nonexistent/file.rs"), "HEAD");
        assert!(result.is_err());
    }

    #[test]
    fn git_diff_file_nonexistent() {
        let result = git_diff_file(&PathBuf::from("/nonexistent/file.rs"));
        assert!(result.is_err());
    }
}
