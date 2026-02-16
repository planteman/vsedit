//! Git-based timeline provider.
//!
//! Parses `git log` output to produce timeline items for a given file path.

use std::fmt;
use std::io;
use std::path::Path;
use std::process::Command;

// ── Core Types ──

/// A single timeline entry derived from a git commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineItem {
    pub timestamp: u64,
    pub message: String,
    pub author: String,
    pub sha: String,
}

impl fmt::Display for TimelineItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} ({}) - {}", self.sha, self.message, self.author, self.timestamp)
    }
}

// ── Errors ──

/// Errors that can occur when building a timeline.
#[derive(Debug)]
pub enum TimelineError {
    /// The git command failed to execute.
    GitExecFailed(io::Error),
    /// The git command returned a non-zero exit code.
    GitFailed(String),
    /// A line from git log could not be parsed.
    ParseError(String),
}

impl fmt::Display for TimelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimelineError::GitExecFailed(e) => write!(f, "failed to execute git: {e}"),
            TimelineError::GitFailed(msg) => write!(f, "git error: {msg}"),
            TimelineError::ParseError(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

// ── Parsing ──

const GIT_LOG_SEP: &str = "\x1f";

/// Parse a single line of `git log` output formatted as `<timestamp>\x1f<sha>\x1f<author>\x1f<message>`.
pub fn parse_git_log_line(line: &str) -> Result<TimelineItem, TimelineError> {
    let parts: Vec<&str> = line.split(GIT_LOG_SEP).collect();
    if parts.len() < 4 {
        return Err(TimelineError::ParseError(format!(
            "expected 4 fields separated by \\x1f, got {}: {:?}",
            parts.len(),
            line
        )));
    }
    let timestamp: u64 = parts[0]
        .trim()
        .parse()
        .map_err(|e| TimelineError::ParseError(format!("invalid timestamp '{}': {e}", parts[0])))?;
    let sha = parts[1].trim().to_string();
    let author = parts[2].trim().to_string();
    let message = parts[3..].join(GIT_LOG_SEP).trim().to_string();
    Ok(TimelineItem {
        timestamp,
        message,
        author,
        sha,
    })
}

/// Parse multi-line `git log` output into timeline items, skipping blank lines.
pub fn parse_git_log_output(output: &str) -> Vec<Result<TimelineItem, TimelineError>> {
    output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(parse_git_log_line)
        .collect()
}

// ── GitTimelineProvider ──

/// Provides timeline items for a file by running `git log`.
#[derive(Debug, Clone)]
pub struct GitTimelineProvider {
    /// Working directory (repository root).
    pub repo_dir: String,
}

impl GitTimelineProvider {
    pub fn new(repo_dir: impl Into<String>) -> Self {
        Self {
            repo_dir: repo_dir.into(),
        }
    }

    /// Run `git log` for the given file path and return parsed timeline items.
    pub fn timeline_for_file(&self, path: &str) -> Result<Vec<TimelineItem>, TimelineError> {
        let format_str = format!("%ct{sep}%H{sep}%an{sep}%s", sep = GIT_LOG_SEP);
        let output = Command::new("git")
            .args([
                "log",
                "--follow",
                &format!("--format={format_str}"),
                "--",
                path,
            ])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(TimelineError::GitExecFailed)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(TimelineError::GitFailed(stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let results = parse_git_log_output(&stdout);
        let mut items = Vec::with_capacity(results.len());
        for r in results {
            items.push(r?);
        }
        Ok(items)
    }
}

/// Convenience function to get timeline items for a file under the given repo directory.
pub fn timeline_for_file(repo_dir: impl AsRef<Path>, path: &str) -> Result<Vec<TimelineItem>, TimelineError> {
    let provider = GitTimelineProvider::new(repo_dir.as_ref().to_string_lossy().to_string());
    provider.timeline_for_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_line() {
        let line = "1700000000\x1fabc123\x1fAlice\x1fFix bug";
        let item = parse_git_log_line(line).unwrap();
        assert_eq!(item.timestamp, 1700000000);
        assert_eq!(item.sha, "abc123");
        assert_eq!(item.author, "Alice");
        assert_eq!(item.message, "Fix bug");
    }

    #[test]
    fn parse_line_with_separator_in_message() {
        let line = "1700000000\x1fabc123\x1fAlice\x1fFix\x1fbug";
        let item = parse_git_log_line(line).unwrap();
        assert_eq!(item.message, "Fix\x1fbug");
    }

    #[test]
    fn parse_invalid_line_too_few_fields() {
        let line = "1700000000\x1fabc123";
        assert!(parse_git_log_line(line).is_err());
    }

    #[test]
    fn parse_invalid_timestamp() {
        let line = "notanumber\x1fabc123\x1fAlice\x1fFix bug";
        assert!(parse_git_log_line(line).is_err());
    }

    #[test]
    fn parse_multi_line_output() {
        let output = "1700000000\x1fabc123\x1fAlice\x1fFirst commit\n\
                       1700000100\x1fdef456\x1fBob\x1fSecond commit\n";
        let results = parse_git_log_output(output);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn parse_multi_line_skips_blank() {
        let output = "\n1700000000\x1fabc123\x1fAlice\x1fCommit\n\n";
        let results = parse_git_log_output(output);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn timeline_item_display() {
        let item = TimelineItem {
            timestamp: 1700000000,
            message: "Fix bug".into(),
            author: "Alice".into(),
            sha: "abc123".into(),
        };
        let s = format!("{item}");
        assert!(s.contains("abc123"));
        assert!(s.contains("Fix bug"));
        assert!(s.contains("Alice"));
    }

    #[test]
    fn timeline_error_display() {
        let e = TimelineError::ParseError("bad line".into());
        assert!(format!("{e}").contains("parse error"));
        let e2 = TimelineError::GitFailed("not a repo".into());
        assert!(format!("{e2}").contains("git error"));
    }

    #[test]
    fn git_timeline_provider_new() {
        let provider = GitTimelineProvider::new("/tmp/repo");
        assert_eq!(provider.repo_dir, "/tmp/repo");
    }

    #[test]
    fn timeline_for_file_in_current_repo() {
        // Use the workspace root (two levels up from the crate dir) so we hit
        // a file that has git history.
        let crate_dir = std::env::current_dir().unwrap();
        let repo_root = crate_dir.join("../..").canonicalize().unwrap_or(crate_dir);
        let result = timeline_for_file(&repo_root, "Cargo.toml");
        match result {
            Ok(items) => {
                // The repo should have at least one commit touching Cargo.toml
                assert!(!items.is_empty());
                for item in &items {
                    assert!(!item.sha.is_empty());
                    assert!(!item.author.is_empty());
                    assert!(item.timestamp > 0);
                }
            }
            Err(TimelineError::GitExecFailed(_)) => {
                // git not available in CI – acceptable
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn timeline_for_nonexistent_file() {
        let crate_dir = std::env::current_dir().unwrap();
        let repo_root = crate_dir.join("../..").canonicalize().unwrap_or(crate_dir);
        let result = timeline_for_file(&repo_root, "this_file_does_not_exist_xyz.txt");
        match result {
            Ok(items) => assert!(items.is_empty()),
            Err(TimelineError::GitExecFailed(_)) => {} // git not available
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
}
