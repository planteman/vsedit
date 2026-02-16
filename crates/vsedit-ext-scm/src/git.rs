//! Git CLI wrapper for SCM operations.
//!
//! Provides a high-level interface over the `git` command-line tool using
//! [`std::process::Command`].

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Error ──────────────────────────────────────────────────────────────────

/// Errors produced by [`GitCli`] operations.
#[derive(Debug)]
pub enum GitError {
    /// The path is not inside a git repository.
    NotAGitRepo,
    /// The git command exited with a non-zero status.
    CommandFailed(String),
    /// Output from git could not be parsed.
    ParseError(String),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAGitRepo => write!(f, "not a git repository"),
            Self::CommandFailed(msg) => write!(f, "git command failed: {msg}"),
            Self::ParseError(msg) => write!(f, "git parse error: {msg}"),
        }
    }
}

impl std::error::Error for GitError {}

// ── Supporting types ───────────────────────────────────────────────────────

/// Status of a single file in the working tree / index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Staged,
}

impl fmt::Display for FileStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Modified => "Modified",
            Self::Added => "Added",
            Self::Deleted => "Deleted",
            Self::Renamed => "Renamed",
            Self::Untracked => "Untracked",
            Self::Staged => "Staged",
        })
    }
}

/// A file together with its [`FileStatus`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitFileStatus {
    pub path: PathBuf,
    pub status: FileStatus,
}

/// A single entry from `git log --oneline`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLogEntry {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

// ── Parsing helpers (pure, tested independently) ───────────────────────────

/// Parse a single `git status --porcelain=v1` line into a [`GitFileStatus`].
pub fn parse_porcelain_line(line: &str) -> Option<GitFileStatus> {
    if line.len() < 4 {
        return None;
    }
    let index = line.as_bytes()[0];
    let worktree = line.as_bytes()[1];
    let path_part = &line[3..];

    // Renames: "R  old -> new"
    if index == b'R' || worktree == b'R' {
        let path = match path_part.rsplit_once(" -> ") {
            Some((_, new)) => new,
            None => path_part,
        };
        return Some(GitFileStatus {
            path: PathBuf::from(path),
            status: FileStatus::Renamed,
        });
    }

    let status = match (index, worktree) {
        (b'?', b'?') => FileStatus::Untracked,
        (b'A', _) => FileStatus::Added,
        (b'D', _) | (_, b'D') => FileStatus::Deleted,
        (b'M', b' ') => FileStatus::Staged,
        (_, b'M') | (b'M', _) => FileStatus::Modified,
        _ if index != b' ' && index != b'?' => FileStatus::Staged,
        _ => FileStatus::Modified,
    };

    Some(GitFileStatus {
        path: PathBuf::from(path_part),
        status,
    })
}

/// Parse full `git status --porcelain=v1` output.
pub fn parse_porcelain(output: &str) -> Vec<GitFileStatus> {
    output.lines().filter_map(parse_porcelain_line).collect()
}

/// Parse `git log --format="%h%x00%s%x00%an%x00%ai" -N` output.
pub fn parse_log_output(output: &str) -> Vec<GitLogEntry> {
    output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '\0').collect();
            if parts.len() == 4 {
                Some(GitLogEntry {
                    sha: parts[0].to_string(),
                    message: parts[1].to_string(),
                    author: parts[2].to_string(),
                    date: parts[3].to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Parse `git log --oneline -N` output into [`GitLogEntry`] items (author/date
/// left empty because oneline format does not include them).
pub fn parse_log_oneline(output: &str) -> Vec<GitLogEntry> {
    output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let (sha, message) = line.split_once(' ')?;
            Some(GitLogEntry {
                sha: sha.to_string(),
                message: message.to_string(),
                author: String::new(),
                date: String::new(),
            })
        })
        .collect()
}

/// Parse `git rev-parse --show-toplevel` output into a [`PathBuf`].
pub fn parse_repo_root(output: &str) -> Option<PathBuf> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

// ── GitCli ─────────────────────────────────────────────────────────────────

/// Git CLI wrapper for SCM operations.
pub struct GitCli {
    repo_root: PathBuf,
}

impl GitCli {
    /// Create a new `GitCli` rooted at `repo_root`.
    pub fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    /// Check whether `path` is inside a git repository.
    pub fn is_git_repo(path: &Path) -> bool {
        Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(path)
            .output()
            .ok()
            .is_some_and(|o| o.status.success())
    }

    /// Find the git repository root starting from `path`.
    pub fn find_repo_root(path: &Path) -> Option<PathBuf> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(path)
            .output()
            .ok()?;
        if output.status.success() {
            parse_repo_root(&String::from_utf8_lossy(&output.stdout))
        } else {
            None
        }
    }

    /// Return the repository root path.
    pub fn root(&self) -> &Path {
        &self.repo_root
    }

    // ── Internal helper ────────────────────────────────────────────────

    fn run(&self, args: &[&str]) -> Result<String, GitError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| GitError::CommandFailed(e.to_string()))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_string())
        } else {
            Err(GitError::CommandFailed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ))
        }
    }

    // ── Public operations ──────────────────────────────────────────────

    /// Get file status (modified, added, deleted, untracked, etc.).
    pub fn status(&self) -> Result<Vec<GitFileStatus>, GitError> {
        let out = self.run(&["status", "--porcelain=v1"])?;
        Ok(parse_porcelain(&out))
    }

    /// Stage a file.
    pub fn stage(&self, path: &Path) -> Result<(), GitError> {
        let p = path.to_string_lossy();
        self.run(&["add", "--", &p])?;
        Ok(())
    }

    /// Unstage a file.
    pub fn unstage(&self, path: &Path) -> Result<(), GitError> {
        let p = path.to_string_lossy();
        self.run(&["reset", "HEAD", "--", &p])?;
        Ok(())
    }

    /// Commit staged changes with the given message.
    pub fn commit(&self, message: &str) -> Result<String, GitError> {
        self.run(&["commit", "-m", message])
    }

    /// Get the unified diff for a file (working tree vs index).
    pub fn diff_file(&self, path: &Path) -> Result<String, GitError> {
        let p = path.to_string_lossy();
        self.run(&["diff", "--", &p])
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> Result<String, GitError> {
        self.run(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    /// Get a short log of the last `count` commits.
    pub fn log(&self, count: usize) -> Result<Vec<GitLogEntry>, GitError> {
        let n = format!("-{count}");
        let out = self.run(&["log", "--oneline", &n])?;
        Ok(parse_log_oneline(&out))
    }

    /// Discard working-tree changes to a file.
    pub fn discard(&self, path: &Path) -> Result<(), GitError> {
        let p = path.to_string_lossy();
        self.run(&["checkout", "--", &p])?;
        Ok(())
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_porcelain_line -----------------------------------------------

    #[test]
    fn parse_modified_worktree() {
        let line = " M src/lib.rs";
        let entry = parse_porcelain_line(line).unwrap();
        assert_eq!(entry.status, FileStatus::Modified);
        assert_eq!(entry.path, PathBuf::from("src/lib.rs"));
    }

    #[test]
    fn parse_staged_modified() {
        let line = "M  src/main.rs";
        let entry = parse_porcelain_line(line).unwrap();
        assert_eq!(entry.status, FileStatus::Staged);
        assert_eq!(entry.path, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn parse_added_file() {
        let line = "A  new_file.rs";
        let entry = parse_porcelain_line(line).unwrap();
        assert_eq!(entry.status, FileStatus::Added);
        assert_eq!(entry.path, PathBuf::from("new_file.rs"));
    }

    #[test]
    fn parse_deleted_index() {
        let line = "D  removed.rs";
        let entry = parse_porcelain_line(line).unwrap();
        assert_eq!(entry.status, FileStatus::Deleted);
        assert_eq!(entry.path, PathBuf::from("removed.rs"));
    }

    #[test]
    fn parse_deleted_worktree() {
        let line = " D gone.rs";
        let entry = parse_porcelain_line(line).unwrap();
        assert_eq!(entry.status, FileStatus::Deleted);
        assert_eq!(entry.path, PathBuf::from("gone.rs"));
    }

    #[test]
    fn parse_untracked() {
        let line = "?? unknown.txt";
        let entry = parse_porcelain_line(line).unwrap();
        assert_eq!(entry.status, FileStatus::Untracked);
        assert_eq!(entry.path, PathBuf::from("unknown.txt"));
    }

    #[test]
    fn parse_renamed() {
        let line = "R  old.rs -> new.rs";
        let entry = parse_porcelain_line(line).unwrap();
        assert_eq!(entry.status, FileStatus::Renamed);
        assert_eq!(entry.path, PathBuf::from("new.rs"));
    }

    #[test]
    fn parse_short_line_returns_none() {
        assert!(parse_porcelain_line("").is_none());
        assert!(parse_porcelain_line("M").is_none());
        assert!(parse_porcelain_line("M ").is_none());
    }

    #[test]
    fn parse_porcelain_multi_line() {
        let output = " M src/lib.rs\n?? todo.txt\nA  new.rs\n";
        let entries = parse_porcelain(&output);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].status, FileStatus::Modified);
        assert_eq!(entries[1].status, FileStatus::Untracked);
        assert_eq!(entries[2].status, FileStatus::Added);
    }

    #[test]
    fn parse_porcelain_empty_output() {
        let entries = parse_porcelain("");
        assert!(entries.is_empty());
    }

    // -- parse_log_oneline --------------------------------------------------

    #[test]
    fn parse_log_oneline_basic() {
        let output = "abc1234 Initial commit\ndef5678 Add README\n";
        let entries = parse_log_oneline(output);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sha, "abc1234");
        assert_eq!(entries[0].message, "Initial commit");
        assert_eq!(entries[1].sha, "def5678");
        assert_eq!(entries[1].message, "Add README");
    }

    #[test]
    fn parse_log_oneline_empty() {
        let entries = parse_log_oneline("");
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_log_oneline_message_with_spaces() {
        let output = "aaa1111 fix: handle edge case in parser\n";
        let entries = parse_log_oneline(output);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "fix: handle edge case in parser");
    }

    // -- parse_log_output (structured format) --------------------------------

    #[test]
    fn parse_log_output_structured() {
        let output =
            "abc1234\0Initial commit\0Alice\02024-01-15 10:00:00 -0500\n\
             def5678\0Add feature\0Bob\02024-01-16 14:30:00 -0500\n";
        let entries = parse_log_output(output);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sha, "abc1234");
        assert_eq!(entries[0].author, "Alice");
        assert_eq!(entries[1].message, "Add feature");
        assert_eq!(entries[1].author, "Bob");
    }

    // -- parse_repo_root ----------------------------------------------------

    #[test]
    fn parse_repo_root_valid() {
        let root = parse_repo_root("/home/user/project\n").unwrap();
        assert_eq!(root, PathBuf::from("/home/user/project"));
    }

    #[test]
    fn parse_repo_root_empty() {
        assert!(parse_repo_root("").is_none());
        assert!(parse_repo_root("  \n").is_none());
    }

    // -- is_git_repo (uses the real filesystem) -----------------------------

    #[test]
    fn is_git_repo_on_cwd() {
        // The vsedit project itself is a git repo, so cwd should be a repo.
        assert!(GitCli::is_git_repo(Path::new(".")));
    }

    // -- Error display ------------------------------------------------------

    #[test]
    fn error_display() {
        assert_eq!(GitError::NotAGitRepo.to_string(), "not a git repository");
        assert_eq!(
            GitError::CommandFailed("oops".into()).to_string(),
            "git command failed: oops"
        );
        assert_eq!(
            GitError::ParseError("bad".into()).to_string(),
            "git parse error: bad"
        );
    }

    // -- FileStatus display -------------------------------------------------

    #[test]
    fn file_status_display() {
        assert_eq!(FileStatus::Modified.to_string(), "Modified");
        assert_eq!(FileStatus::Added.to_string(), "Added");
        assert_eq!(FileStatus::Deleted.to_string(), "Deleted");
        assert_eq!(FileStatus::Renamed.to_string(), "Renamed");
        assert_eq!(FileStatus::Untracked.to_string(), "Untracked");
        assert_eq!(FileStatus::Staged.to_string(), "Staged");
    }
}
