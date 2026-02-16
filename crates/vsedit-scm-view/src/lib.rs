//! Source control view — SCM sidebar equivalent to VS Code's Git panel.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

// ---------------------------------------------------------------------------
// ScmFileStatus
// ---------------------------------------------------------------------------

/// Status of a file in the working tree or index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScmFileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
    Ignored,
}

impl ScmFileStatus {
    /// Single-character icon for the status.
    pub fn icon(self) -> char {
        match self {
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Untracked => '?',
            Self::Conflicted => '!',
            Self::Ignored => 'I',
        }
    }

    /// Colour used when rendering the icon.
    pub fn color(self) -> Color {
        match self {
            Self::Modified => Color::Yellow,
            Self::Added => Color::Green,
            Self::Deleted => Color::Red,
            Self::Renamed => Color::Cyan,
            Self::Untracked => Color::Gray,
            Self::Conflicted => Color::Red,
            Self::Ignored => Color::DarkGray,
        }
    }

    /// Style for the status icon.
    pub fn style(self) -> Style {
        let s = Style::default().fg(self.color());
        if self == Self::Conflicted {
            s.add_modifier(Modifier::BOLD)
        } else {
            s
        }
    }
}

impl fmt::Display for ScmFileStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Modified => "Modified",
            Self::Added => "Added",
            Self::Deleted => "Deleted",
            Self::Renamed => "Renamed",
            Self::Untracked => "Untracked",
            Self::Conflicted => "Conflicted",
            Self::Ignored => "Ignored",
        })
    }
}

// ---------------------------------------------------------------------------
// ScmFileChange
// ---------------------------------------------------------------------------

/// A single changed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScmFileChange {
    pub path: PathBuf,
    pub status: ScmFileStatus,
    /// Original path before a rename, if applicable.
    pub original_path: Option<PathBuf>,
}

impl ScmFileChange {
    pub fn new(path: impl Into<PathBuf>, status: ScmFileStatus) -> Self {
        Self {
            path: path.into(),
            status,
            original_path: None,
        }
    }

    pub fn with_original(mut self, original: impl Into<PathBuf>) -> Self {
        self.original_path = Some(original.into());
        self
    }
}

// ---------------------------------------------------------------------------
// ScmGroup
// ---------------------------------------------------------------------------

/// A logical group of changes (e.g. "Staged Changes", "Changes").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScmGroup {
    pub id: String,
    pub label: String,
    pub changes: Vec<ScmFileChange>,
    pub is_expanded: bool,
}

impl ScmGroup {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            changes: Vec::new(),
            is_expanded: true,
        }
    }

    pub fn add_change(&mut self, change: ScmFileChange) {
        self.changes.push(change);
    }

    pub fn toggle_expanded(&mut self) {
        self.is_expanded = !self.is_expanded;
    }

    /// Total visible rows: 1 for the header + children when expanded.
    pub fn visible_rows(&self) -> usize {
        if self.is_expanded {
            1 + self.changes.len()
        } else {
            1
        }
    }
}

// ---------------------------------------------------------------------------
// ScmProvider trait
// ---------------------------------------------------------------------------

/// Abstraction over a source-control provider.
pub trait ScmProvider {
    fn name(&self) -> &str;
    fn root_path(&self) -> &Path;
    fn get_groups(&self) -> Vec<ScmGroup>;
    fn stage(&self, paths: &[&Path]);
    fn unstage(&self, paths: &[&Path]);
    fn commit(&self, message: &str);
    fn get_branch(&self) -> Option<String>;
    /// Returns `(ahead, behind)` relative to the upstream.
    fn get_commit_count(&self) -> (usize, usize);
}

// ---------------------------------------------------------------------------
// GitScmProvider
// ---------------------------------------------------------------------------

/// SCM provider backed by the `git` CLI.
pub struct GitScmProvider {
    root: PathBuf,
}

impl GitScmProvider {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn git(&self, args: &[&str]) -> Option<String> {
        Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim_end().to_string())
    }
}

/// Parse a single `git status --porcelain` line into an `ScmFileChange`.
pub fn parse_porcelain_line(line: &str) -> Option<ScmFileChange> {
    if line.len() < 4 {
        return None;
    }
    let index = line.as_bytes()[0];
    let worktree = line.as_bytes()[1];
    let path_part = &line[3..];

    // Renames: "R  old -> new"
    if index == b'R' || worktree == b'R' {
        let parts: Vec<&str> = path_part.splitn(2, " -> ").collect();
        return if parts.len() == 2 {
            Some(
                ScmFileChange::new(parts[1], ScmFileStatus::Renamed)
                    .with_original(parts[0]),
            )
        } else {
            Some(ScmFileChange::new(path_part, ScmFileStatus::Renamed))
        };
    }

    let status = match (index, worktree) {
        (b'U', _) | (_, b'U') | (b'A', b'A') | (b'D', b'D') => {
            ScmFileStatus::Conflicted
        }
        (b'?', b'?') => ScmFileStatus::Untracked,
        (b'!', b'!') => ScmFileStatus::Ignored,
        (b'A', _) => ScmFileStatus::Added,
        (b'D', _) | (_, b'D') => ScmFileStatus::Deleted,
        (b'M', _) | (_, b'M') | (b'T', _) | (_, b'T') => {
            ScmFileStatus::Modified
        }
        _ => ScmFileStatus::Modified,
    };

    Some(ScmFileChange::new(path_part, status))
}

/// Parse the full output of `git status --porcelain` into groups.
pub fn parse_porcelain(output: &str) -> Vec<ScmGroup> {
    let mut staged = ScmGroup::new("staged", "Staged Changes");
    let mut unstaged = ScmGroup::new("changes", "Changes");
    let mut untracked = ScmGroup::new("untracked", "Untracked");

    for line in output.lines() {
        if line.len() < 3 {
            continue;
        }
        let index = line.as_bytes()[0];
        let worktree = line.as_bytes()[1];

        if let Some(change) = parse_porcelain_line(line) {
            match (index, worktree) {
                (b'?', b'?') => untracked.add_change(change),
                (b'!', b'!') => {} // skip ignored
                _ if index != b' ' && index != b'?' => {
                    staged.add_change(change);
                }
                _ => {
                    unstaged.add_change(change);
                }
            }
        }
    }

    let mut groups = Vec::new();
    if !staged.changes.is_empty() {
        groups.push(staged);
    }
    if !unstaged.changes.is_empty() {
        groups.push(unstaged);
    }
    if !untracked.changes.is_empty() {
        groups.push(untracked);
    }
    groups
}

/// Parse `git rev-list --left-right --count @{u}...HEAD` output.
pub fn parse_ahead_behind(output: &str) -> (usize, usize) {
    let parts: Vec<&str> = output.split_whitespace().collect();
    if parts.len() == 2 {
        let behind = parts[0].parse().unwrap_or(0);
        let ahead = parts[1].parse().unwrap_or(0);
        (ahead, behind)
    } else {
        (0, 0)
    }
}

// ---------------------------------------------------------------------------
// FileStatus (fine-grained status enum)
// ---------------------------------------------------------------------------

/// Fine-grained status for a file tracked by git.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileStatus {
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Unmerged,
}

impl FileStatus {
    /// Single-character indicator.
    pub fn indicator(self) -> char {
        match self {
            Self::Untracked => '?',
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Copied => 'C',
            Self::Unmerged => 'U',
        }
    }
}

impl fmt::Display for FileStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Untracked => "Untracked",
            Self::Modified => "Modified",
            Self::Added => "Added",
            Self::Deleted => "Deleted",
            Self::Renamed => "Renamed",
            Self::Copied => "Copied",
            Self::Unmerged => "Unmerged",
        })
    }
}

// ---------------------------------------------------------------------------
// StatusEntry (parsed from `git status --porcelain=v1`)
// ---------------------------------------------------------------------------

/// A single entry from `git status --porcelain=v1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub path: PathBuf,
    pub index_status: FileStatus,
    pub worktree_status: FileStatus,
    /// Original path for renames/copies.
    pub original_path: Option<PathBuf>,
}

/// Parse a porcelain v1 status character into a `FileStatus`.
fn char_to_file_status(ch: u8) -> FileStatus {
    match ch {
        b'M' | b'T' => FileStatus::Modified,
        b'A' => FileStatus::Added,
        b'D' => FileStatus::Deleted,
        b'R' => FileStatus::Renamed,
        b'C' => FileStatus::Copied,
        b'U' => FileStatus::Unmerged,
        b'?' => FileStatus::Untracked,
        _ => FileStatus::Untracked,
    }
}

/// Parse `git status --porcelain=v1` output into `StatusEntry` items.
pub fn parse_status_porcelain(output: &str) -> Vec<StatusEntry> {
    output
        .lines()
        .filter(|l| l.len() >= 4)
        .filter_map(|line| {
            let idx = line.as_bytes()[0];
            let wt = line.as_bytes()[1];
            let path_part = &line[3..];

            if idx == b'?' && wt == b'?' {
                return Some(StatusEntry {
                    path: PathBuf::from(path_part),
                    index_status: FileStatus::Untracked,
                    worktree_status: FileStatus::Untracked,
                    original_path: None,
                });
            }

            // Handle renames/copies: "R  old -> new" or "C  old -> new"
            if idx == b'R' || idx == b'C' || wt == b'R' || wt == b'C' {
                let parts: Vec<&str> = path_part.splitn(2, " -> ").collect();
                let (new_path, orig) = if parts.len() == 2 {
                    (parts[1], Some(PathBuf::from(parts[0])))
                } else {
                    (path_part, None)
                };
                return Some(StatusEntry {
                    path: PathBuf::from(new_path),
                    index_status: char_to_file_status(idx),
                    worktree_status: char_to_file_status(wt),
                    original_path: orig,
                });
            }

            let index_status = if idx == b' ' {
                FileStatus::Untracked
            } else {
                char_to_file_status(idx)
            };
            let worktree_status = if wt == b' ' {
                FileStatus::Untracked
            } else {
                char_to_file_status(wt)
            };

            Some(StatusEntry {
                path: PathBuf::from(path_part),
                index_status,
                worktree_status,
                original_path: None,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// DiffLine / DiffHunk (parsed unified diff)
// ---------------------------------------------------------------------------

/// A single line in a diff hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

/// A parsed hunk from unified diff output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub lines: Vec<DiffLine>,
}

/// Parse `git diff --unified=3` (or `--cached`) output into `DiffHunk` structs.
pub fn parse_diff_hunks(diff_output: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut current: Option<DiffHunk> = None;

    for line in diff_output.lines() {
        if let Some(header) = line.strip_prefix("@@ ") {
            // Flush previous hunk.
            if let Some(h) = current.take() {
                hunks.push(h);
            }
            // Parse "@@ -old_start,old_count +new_start,new_count @@"
            if let Some((old, new)) = parse_hunk_header(header) {
                current = Some(DiffHunk {
                    old_start: old.0,
                    old_count: old.1,
                    new_start: new.0,
                    new_count: new.1,
                    lines: Vec::new(),
                });
            }
        } else if let Some(ref mut hunk) = current {
            if let Some(rest) = line.strip_prefix('+') {
                hunk.lines.push(DiffLine::Added(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix('-') {
                hunk.lines.push(DiffLine::Removed(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix(' ') {
                hunk.lines.push(DiffLine::Context(rest.to_string()));
            } else if line.is_empty() || !line.starts_with('\\') {
                // Context line with no prefix (empty line) or non-special line.
                hunk.lines.push(DiffLine::Context(line.to_string()));
            }
        }
    }
    if let Some(h) = current {
        hunks.push(h);
    }
    hunks
}

/// Parse a hunk header like `-1,3 +1,4 @@` into ((old_start, old_count), (new_start, new_count)).
fn parse_hunk_header(header: &str) -> Option<((u32, u32), (u32, u32))> {
    // Header format: "-old_start,old_count +new_start,new_count @@..."
    let parts: Vec<&str> = header.splitn(2, " @@").collect();
    let range_part = parts.first()?;
    let mut ranges = range_part.split_whitespace();

    let old_range = ranges.next()?.strip_prefix('-')?;
    let new_range = ranges.next()?.strip_prefix('+')?;

    let old = parse_range(old_range);
    let new = parse_range(new_range);

    Some((old, new))
}

fn parse_range(range: &str) -> (u32, u32) {
    if let Some((start, count)) = range.split_once(',') {
        (
            start.parse().unwrap_or(0),
            count.parse().unwrap_or(0),
        )
    } else {
        (range.parse().unwrap_or(0), 1)
    }
}

// ---------------------------------------------------------------------------
// LogEntry
// ---------------------------------------------------------------------------

/// A single commit from `git log --oneline`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub hash: String,
    pub message: String,
}

/// Parse `git log --oneline -n` output into `LogEntry` items.
pub fn parse_log_oneline(output: &str) -> Vec<LogEntry> {
    output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let (hash, msg) = line.split_once(' ')?;
            Some(LogEntry {
                hash: hash.to_string(),
                message: msg.to_string(),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// GitRepository
// ---------------------------------------------------------------------------

/// Error type for git operations.
#[derive(Debug)]
pub enum GitError {
    /// The git command failed with a non-zero exit code.
    CommandFailed { stderr: String },
    /// An I/O error occurred launching or communicating with git.
    Io(io::Error),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandFailed { stderr } => write!(f, "git: {stderr}"),
            Self::Io(e) => write!(f, "git I/O: {e}"),
        }
    }
}

impl std::error::Error for GitError {}

impl From<io::Error> for GitError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// High-level wrapper around the `git` CLI for repository operations.
pub struct GitRepository {
    root: PathBuf,
}

impl GitRepository {
    /// Open a repository at the given root directory.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Root directory of this repository.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Run a git command and return stdout on success.
    fn run(&self, args: &[&str]) -> Result<String, GitError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(GitError::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            })
        }
    }

    /// Run a git command preserving leading whitespace in output lines.
    fn run_raw(&self, args: &[&str]) -> Result<String, GitError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_string())
        } else {
            Err(GitError::CommandFailed {
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            })
        }
    }

    /// Run `git status --porcelain=v1` and parse into `StatusEntry` items.
    pub fn git_status(&self) -> Result<Vec<StatusEntry>, GitError> {
        let out = self.run_raw(&["status", "--porcelain=v1"])?;
        Ok(parse_status_porcelain(&out))
    }

    /// Get `git diff` output for a specific file (unstaged changes).
    pub fn git_diff(&self, file: &Path) -> Result<Vec<DiffHunk>, GitError> {
        let path_str = file.to_string_lossy();
        let out = self.run_raw(&["diff", "--unified=3", "--", &path_str])?;
        Ok(parse_diff_hunks(&out))
    }

    /// Get `git diff --cached` output for a specific file (staged changes).
    pub fn git_diff_staged(&self, file: &Path) -> Result<Vec<DiffHunk>, GitError> {
        let path_str = file.to_string_lossy();
        let out =
            self.run_raw(&["diff", "--cached", "--unified=3", "--", &path_str])?;
        Ok(parse_diff_hunks(&out))
    }

    /// Get the current branch name.
    pub fn git_branch(&self) -> Result<String, GitError> {
        self.run(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    /// Get the last `n` commits as `LogEntry` items.
    pub fn git_log(&self, n: usize) -> Result<Vec<LogEntry>, GitError> {
        let n_str = format!("-{n}");
        let out = self.run(&["log", "--oneline", &n_str])?;
        Ok(parse_log_oneline(&out))
    }

    /// Stage a file with `git add`.
    pub fn git_stage(&self, file: &Path) -> Result<(), GitError> {
        let path_str = file.to_string_lossy();
        self.run(&["add", "--", &path_str])?;
        Ok(())
    }

    /// Unstage a file with `git restore --staged`.
    pub fn git_unstage(&self, file: &Path) -> Result<(), GitError> {
        let path_str = file.to_string_lossy();
        self.run(&["restore", "--staged", "--", &path_str])?;
        Ok(())
    }

    /// Commit staged changes with the given message.
    pub fn git_commit(&self, message: &str) -> Result<(), GitError> {
        self.run(&["commit", "-m", message])?;
        Ok(())
    }

    /// Discard working-tree changes to a file.
    pub fn git_discard(&self, file: &Path) -> Result<(), GitError> {
        let path_str = file.to_string_lossy();
        self.run(&["checkout", "--", &path_str])?;
        Ok(())
    }

    /// Return grouped changes: (staged, unstaged, untracked).
    pub fn grouped_status(
        &self,
    ) -> Result<(Vec<StatusEntry>, Vec<StatusEntry>, Vec<StatusEntry>), GitError> {
        let entries = self.git_status()?;
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for entry in entries {
            match (entry.index_status, entry.worktree_status) {
                (FileStatus::Untracked, FileStatus::Untracked) => {
                    untracked.push(entry);
                }
                (idx, wt) => {
                    // If index has a real status, it's staged.
                    if idx != FileStatus::Untracked {
                        staged.push(entry.clone());
                    }
                    // If worktree has a real change, it's unstaged.
                    if wt != FileStatus::Untracked
                        && !(idx != FileStatus::Untracked
                            && wt == FileStatus::Untracked)
                    {
                        unstaged.push(entry);
                    }
                }
            }
        }

        Ok((staged, unstaged, untracked))
    }
}

impl ScmProvider for GitScmProvider {
    fn name(&self) -> &str {
        "git"
    }

    fn root_path(&self) -> &Path {
        &self.root
    }

    fn get_groups(&self) -> Vec<ScmGroup> {
        self.git(&["status", "--porcelain"])
            .map_or_else(Vec::new, |out| parse_porcelain(&out))
    }

    fn stage(&self, paths: &[&Path]) {
        let strs: Vec<&str> =
            paths.iter().filter_map(|p| p.to_str()).collect();
        if !strs.is_empty() {
            let mut args = vec!["add", "--"];
            args.extend(strs);
            let _ = self.git(&args);
        }
    }

    fn unstage(&self, paths: &[&Path]) {
        let strs: Vec<&str> =
            paths.iter().filter_map(|p| p.to_str()).collect();
        if !strs.is_empty() {
            let mut args = vec!["reset", "HEAD", "--"];
            args.extend(strs);
            let _ = self.git(&args);
        }
    }

    fn commit(&self, message: &str) {
        let _ = self.git(&["commit", "-m", message]);
    }

    fn get_branch(&self) -> Option<String> {
        self.git(&["branch", "--show-current"])
            .filter(|b| !b.is_empty())
    }

    fn get_commit_count(&self) -> (usize, usize) {
        self.git(&["rev-list", "--left-right", "--count", "@{u}...HEAD"])
            .map_or((0, 0), |out| parse_ahead_behind(&out))
    }
}

// ---------------------------------------------------------------------------
// ScmView (UI state + rendering)
// ---------------------------------------------------------------------------

/// UI state for the Source Control sidebar.
pub struct ScmView {
    pub commit_message: String,
    pub selected_index: usize,
    pub scroll_offset: usize,
}

impl ScmView {
    pub fn new() -> Self {
        Self {
            commit_message: String::new(),
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    /// Render the SCM view into a ratatui buffer.
    pub fn render(
        &self,
        groups: &[ScmGroup],
        branch: Option<&str>,
        area: Rect,
        buf: &mut Buffer,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut y = area.y;

        // -- Branch info line -----------------------------------------------
        if y < area.y + area.height {
            let branch_text = branch.unwrap_or("(detached)");
            let label = format!(" ⎇ {branch_text}");
            let style = Style::default().fg(Color::Cyan);
            self.put_line(buf, area.x, y, area.width, &label, style);
            y += 1;
        }

        // -- Commit message input -------------------------------------------
        if y < area.y + area.height {
            let msg = if self.commit_message.is_empty() {
                "Message (Enter to commit)".to_string()
            } else {
                self.commit_message.clone()
            };
            let msg_style = if self.commit_message.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            self.put_line(buf, area.x, y, area.width, &format!(" {msg}"), msg_style);
            y += 1;
        }

        // -- Separator ------------------------------------------------------
        if y < area.y + area.height {
            let sep: String =
                std::iter::repeat_n('─', area.width as usize).collect();
            self.put_line(
                buf,
                area.x,
                y,
                area.width,
                &sep,
                Style::default().fg(Color::DarkGray),
            );
            y += 1;
        }

        // -- Groups and changes ---------------------------------------------
        let mut flat_index: usize = 0;
        for group in groups {
            if y >= area.y + area.height {
                break;
            }

            // Group header
            if flat_index >= self.scroll_offset {
                let arrow = if group.is_expanded { "▼" } else { "▶" };
                let header = format!(
                    " {arrow} {} ({})",
                    group.label,
                    group.changes.len()
                );
                let style = if flat_index == self.selected_index {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::White)
                };
                self.put_line(buf, area.x, y, area.width, &header, style);
                y += 1;
            }
            flat_index += 1;

            // Files in group
            if group.is_expanded {
                for change in &group.changes {
                    if y >= area.y + area.height {
                        break;
                    }
                    if flat_index >= self.scroll_offset {
                        let icon = change.status.icon();
                        let icon_style = change.status.style();
                        let filename = change
                            .path
                            .file_name()
                            .map_or_else(
                                || change.path.to_string_lossy().into_owned(),
                                |n| n.to_string_lossy().into_owned(),
                            );
                        let dir = change
                            .path
                            .parent()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default();

                        let selected =
                            flat_index == self.selected_index;
                        let bg = if selected {
                            Color::DarkGray
                        } else {
                            Color::Reset
                        };

                        // "  M filename  dir"
                        let x = area.x;
                        let max_x = area.x + area.width;

                        // Indent
                        let col = x + 2;
                        if col < max_x {
                            buf[(col, y)]
                                .set_char(icon)
                                .set_style(icon_style.bg(bg));
                        }

                        // Space + filename
                        let name_start = col + 2;
                        for (i, ch) in filename.chars().enumerate() {
                            let cx = name_start + i as u16;
                            if cx >= max_x {
                                break;
                            }
                            buf[(cx, y)].set_char(ch).set_style(
                                Style::default().fg(Color::White).bg(bg),
                            );
                        }

                        // Dir path (dimmed)
                        if !dir.is_empty() {
                            let dir_start =
                                name_start + filename.len() as u16 + 1;
                            for (i, ch) in dir.chars().enumerate() {
                                let cx = dir_start + i as u16;
                                if cx >= max_x {
                                    break;
                                }
                                buf[(cx, y)].set_char(ch).set_style(
                                    Style::default()
                                        .fg(Color::DarkGray)
                                        .bg(bg),
                                );
                            }
                        }

                        // Fill background for selection
                        if selected {
                            for cx in area.x..max_x {
                                let cell = &mut buf[(cx, y)];
                                if cell.symbol() == " " {
                                    cell.set_style(
                                        Style::default().bg(bg),
                                    );
                                }
                            }
                        }

                        y += 1;
                    }
                    flat_index += 1;
                }
            }
        }
    }

    /// Write a truncated line into the buffer.
    fn put_line(
        &self,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        width: u16,
        text: &str,
        style: Style,
    ) {
        for (i, ch) in text.chars().enumerate() {
            let cx = x + i as u16;
            if cx >= x + width {
                break;
            }
            buf[(cx, y)].set_char(ch).set_style(style);
        }
    }
}

impl Default for ScmView {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- ScmFileStatus tests -----------------------------------------------

    #[test]
    fn file_status_display() {
        assert_eq!(ScmFileStatus::Modified.to_string(), "Modified");
        assert_eq!(ScmFileStatus::Added.to_string(), "Added");
        assert_eq!(ScmFileStatus::Deleted.to_string(), "Deleted");
        assert_eq!(ScmFileStatus::Renamed.to_string(), "Renamed");
        assert_eq!(ScmFileStatus::Untracked.to_string(), "Untracked");
        assert_eq!(ScmFileStatus::Conflicted.to_string(), "Conflicted");
        assert_eq!(ScmFileStatus::Ignored.to_string(), "Ignored");
    }

    #[test]
    fn file_status_icons() {
        assert_eq!(ScmFileStatus::Modified.icon(), 'M');
        assert_eq!(ScmFileStatus::Added.icon(), 'A');
        assert_eq!(ScmFileStatus::Deleted.icon(), 'D');
        assert_eq!(ScmFileStatus::Renamed.icon(), 'R');
        assert_eq!(ScmFileStatus::Untracked.icon(), '?');
        assert_eq!(ScmFileStatus::Conflicted.icon(), '!');
        assert_eq!(ScmFileStatus::Ignored.icon(), 'I');
    }

    #[test]
    fn file_status_colors() {
        assert_eq!(ScmFileStatus::Modified.color(), Color::Yellow);
        assert_eq!(ScmFileStatus::Added.color(), Color::Green);
        assert_eq!(ScmFileStatus::Deleted.color(), Color::Red);
        assert_eq!(ScmFileStatus::Renamed.color(), Color::Cyan);
        assert_eq!(ScmFileStatus::Untracked.color(), Color::Gray);
        assert_eq!(ScmFileStatus::Conflicted.color(), Color::Red);
    }

    #[test]
    fn conflicted_style_is_bold() {
        let style = ScmFileStatus::Conflicted.style();
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    // -- ScmGroup tests ----------------------------------------------------

    #[test]
    fn group_creation_and_add() {
        let mut group = ScmGroup::new("staged", "Staged Changes");
        assert!(group.is_expanded);
        assert!(group.changes.is_empty());

        group.add_change(ScmFileChange::new("src/main.rs", ScmFileStatus::Modified));
        assert_eq!(group.changes.len(), 1);
        assert_eq!(group.changes[0].status, ScmFileStatus::Modified);
    }

    #[test]
    fn group_toggle_and_visible_rows() {
        let mut group = ScmGroup::new("changes", "Changes");
        group.add_change(ScmFileChange::new("a.rs", ScmFileStatus::Modified));
        group.add_change(ScmFileChange::new("b.rs", ScmFileStatus::Added));

        assert_eq!(group.visible_rows(), 3); // header + 2 files
        group.toggle_expanded();
        assert!(!group.is_expanded);
        assert_eq!(group.visible_rows(), 1); // header only
    }

    // -- ScmFileChange tests -----------------------------------------------

    #[test]
    fn file_change_with_rename() {
        let change = ScmFileChange::new("new_name.rs", ScmFileStatus::Renamed)
            .with_original("old_name.rs");
        assert_eq!(change.path, PathBuf::from("new_name.rs"));
        assert_eq!(
            change.original_path,
            Some(PathBuf::from("old_name.rs"))
        );
    }

    // -- Porcelain parsing (mock git output) --------------------------------

    #[test]
    fn parse_porcelain_modified() {
        let line = " M src/lib.rs";
        let change = parse_porcelain_line(line).unwrap();
        assert_eq!(change.status, ScmFileStatus::Modified);
        assert_eq!(change.path, PathBuf::from("src/lib.rs"));
    }

    #[test]
    fn parse_porcelain_added() {
        let line = "A  new_file.rs";
        let change = parse_porcelain_line(line).unwrap();
        assert_eq!(change.status, ScmFileStatus::Added);
    }

    #[test]
    fn parse_porcelain_deleted() {
        let line = " D removed.rs";
        let change = parse_porcelain_line(line).unwrap();
        assert_eq!(change.status, ScmFileStatus::Deleted);
    }

    #[test]
    fn parse_porcelain_untracked() {
        let line = "?? unknown.txt";
        let change = parse_porcelain_line(line).unwrap();
        assert_eq!(change.status, ScmFileStatus::Untracked);
        assert_eq!(change.path, PathBuf::from("unknown.txt"));
    }

    #[test]
    fn parse_porcelain_rename() {
        let line = "R  old.rs -> new.rs";
        let change = parse_porcelain_line(line).unwrap();
        assert_eq!(change.status, ScmFileStatus::Renamed);
        assert_eq!(change.path, PathBuf::from("new.rs"));
        assert_eq!(
            change.original_path,
            Some(PathBuf::from("old.rs"))
        );
    }

    #[test]
    fn parse_porcelain_conflict() {
        let line = "UU conflicted.rs";
        let change = parse_porcelain_line(line).unwrap();
        assert_eq!(change.status, ScmFileStatus::Conflicted);
    }

    #[test]
    fn parse_porcelain_full_output() {
        let output = "M  staged.rs\n M unstaged.rs\nA  added.rs\n?? untracked.txt\n";
        let groups = parse_porcelain(output);
        assert_eq!(groups.len(), 3);

        let staged = &groups[0];
        assert_eq!(staged.id, "staged");
        assert_eq!(staged.changes.len(), 2); // M staged.rs + A added.rs

        let unstaged = &groups[1];
        assert_eq!(unstaged.id, "changes");
        assert_eq!(unstaged.changes.len(), 1);

        let untracked = &groups[2];
        assert_eq!(untracked.id, "untracked");
        assert_eq!(untracked.changes.len(), 1);
    }

    #[test]
    fn parse_ahead_behind_normal() {
        assert_eq!(parse_ahead_behind("3\t5"), (5, 3));
        assert_eq!(parse_ahead_behind("0\t0"), (0, 0));
    }

    #[test]
    fn parse_ahead_behind_empty() {
        assert_eq!(parse_ahead_behind(""), (0, 0));
    }

    // -- ScmView state tests -----------------------------------------------

    #[test]
    fn scm_view_default() {
        let view = ScmView::new();
        assert!(view.commit_message.is_empty());
        assert_eq!(view.selected_index, 0);
        assert_eq!(view.scroll_offset, 0);
    }

    #[test]
    fn scm_view_render_empty() {
        let view = ScmView::new();
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        view.render(&[], None, area, &mut buf);
        // Should not panic; branch line shows "(detached)"
        let first_line: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        assert!(first_line.contains("detached"));
    }

    #[test]
    fn scm_view_render_with_groups() {
        let view = ScmView::new();
        let mut group = ScmGroup::new("changes", "Changes");
        group.add_change(ScmFileChange::new(
            "src/main.rs",
            ScmFileStatus::Modified,
        ));
        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);
        view.render(&[group], Some("main"), area, &mut buf);

        // Branch line contains "main"
        let first_line: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        assert!(first_line.contains("main"));
    }

    #[test]
    fn scm_view_render_zero_area() {
        let view = ScmView::new();
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(area);
        view.render(&[], None, area, &mut buf);
        // Should not panic
    }

    // -- GitScmProvider construction ----------------------------------------

    #[test]
    fn git_provider_name() {
        let provider = GitScmProvider::new("/tmp");
        assert_eq!(provider.name(), "git");
        assert_eq!(provider.root_path(), Path::new("/tmp"));
    }

    // -- FileStatus tests ---------------------------------------------------

    #[test]
    fn file_status_indicators() {
        assert_eq!(FileStatus::Untracked.indicator(), '?');
        assert_eq!(FileStatus::Modified.indicator(), 'M');
        assert_eq!(FileStatus::Added.indicator(), 'A');
        assert_eq!(FileStatus::Deleted.indicator(), 'D');
        assert_eq!(FileStatus::Renamed.indicator(), 'R');
        assert_eq!(FileStatus::Copied.indicator(), 'C');
        assert_eq!(FileStatus::Unmerged.indicator(), 'U');
    }

    #[test]
    fn file_status_display_all() {
        assert_eq!(FileStatus::Untracked.to_string(), "Untracked");
        assert_eq!(FileStatus::Modified.to_string(), "Modified");
        assert_eq!(FileStatus::Added.to_string(), "Added");
        assert_eq!(FileStatus::Deleted.to_string(), "Deleted");
        assert_eq!(FileStatus::Renamed.to_string(), "Renamed");
        assert_eq!(FileStatus::Copied.to_string(), "Copied");
        assert_eq!(FileStatus::Unmerged.to_string(), "Unmerged");
    }

    // -- parse_status_porcelain tests ---------------------------------------

    #[test]
    fn parse_status_modified_worktree() {
        let entries = parse_status_porcelain(" M src/lib.rs\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index_status, FileStatus::Untracked);
        assert_eq!(entries[0].worktree_status, FileStatus::Modified);
        assert_eq!(entries[0].path, PathBuf::from("src/lib.rs"));
    }

    #[test]
    fn parse_status_modified_index() {
        let entries = parse_status_porcelain("M  staged.rs\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index_status, FileStatus::Modified);
    }

    #[test]
    fn parse_status_both_modified() {
        let entries = parse_status_porcelain("MM both.rs\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index_status, FileStatus::Modified);
        assert_eq!(entries[0].worktree_status, FileStatus::Modified);
    }

    #[test]
    fn parse_status_untracked() {
        let entries = parse_status_porcelain("?? new_file.txt\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index_status, FileStatus::Untracked);
        assert_eq!(entries[0].worktree_status, FileStatus::Untracked);
    }

    #[test]
    fn parse_status_added() {
        let entries = parse_status_porcelain("A  brand_new.rs\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index_status, FileStatus::Added);
    }

    #[test]
    fn parse_status_deleted() {
        let entries = parse_status_porcelain(" D gone.rs\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].worktree_status, FileStatus::Deleted);
    }

    #[test]
    fn parse_status_renamed() {
        let entries = parse_status_porcelain("R  old.rs -> new.rs\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index_status, FileStatus::Renamed);
        assert_eq!(entries[0].path, PathBuf::from("new.rs"));
        assert_eq!(entries[0].original_path, Some(PathBuf::from("old.rs")));
    }

    #[test]
    fn parse_status_mixed_output() {
        let output = "M  staged.rs\n M unstaged.rs\nA  added.rs\n?? untracked.txt\n D gone.rs\n";
        let entries = parse_status_porcelain(output);
        assert_eq!(entries.len(), 5);
    }

    // -- DiffHunk / parse_diff_hunks tests ----------------------------------

    #[test]
    fn parse_single_hunk() {
        let diff = "\
diff --git a/foo.rs b/foo.rs
index abc..def 100644
--- a/foo.rs
+++ b/foo.rs
@@ -1,3 +1,4 @@
 line1
-old line
+new line
+added line
 line3
";
        let hunks = parse_diff_hunks(diff);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].old_start, 1);
        assert_eq!(hunks[0].old_count, 3);
        assert_eq!(hunks[0].new_start, 1);
        assert_eq!(hunks[0].new_count, 4);
        assert_eq!(hunks[0].lines.len(), 5);
        assert_eq!(hunks[0].lines[0], DiffLine::Context("line1".into()));
        assert_eq!(hunks[0].lines[1], DiffLine::Removed("old line".into()));
        assert_eq!(hunks[0].lines[2], DiffLine::Added("new line".into()));
        assert_eq!(hunks[0].lines[3], DiffLine::Added("added line".into()));
        assert_eq!(hunks[0].lines[4], DiffLine::Context("line3".into()));
    }

    #[test]
    fn parse_multiple_hunks() {
        let diff = "\
@@ -1,2 +1,2 @@
-aaa
+bbb
 ctx
@@ -10,3 +10,3 @@
 ctx
-old
+new
 ctx
";
        let hunks = parse_diff_hunks(diff);
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].old_start, 1);
        assert_eq!(hunks[1].old_start, 10);
    }

    #[test]
    fn parse_hunk_single_line_range() {
        let diff = "@@ -5 +5 @@\n-removed\n+added\n";
        let hunks = parse_diff_hunks(diff);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].old_start, 5);
        assert_eq!(hunks[0].old_count, 1);
        assert_eq!(hunks[0].new_start, 5);
        assert_eq!(hunks[0].new_count, 1);
    }

    #[test]
    fn parse_empty_diff() {
        let hunks = parse_diff_hunks("");
        assert!(hunks.is_empty());
    }

    // -- LogEntry / parse_log_oneline tests ---------------------------------

    #[test]
    fn parse_log_entries() {
        let output = "abc1234 Initial commit\ndef5678 Add feature\n";
        let entries = parse_log_oneline(output);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].hash, "abc1234");
        assert_eq!(entries[0].message, "Initial commit");
        assert_eq!(entries[1].hash, "def5678");
        assert_eq!(entries[1].message, "Add feature");
    }

    #[test]
    fn parse_log_empty() {
        let entries = parse_log_oneline("");
        assert!(entries.is_empty());
    }

    // -- GitError display ---------------------------------------------------

    #[test]
    fn git_error_display() {
        let err = GitError::CommandFailed {
            stderr: "fatal: not a git repo".into(),
        };
        assert!(err.to_string().contains("not a git repo"));
    }

    // -- GitRepository construction -----------------------------------------

    #[test]
    fn git_repository_root() {
        let repo = GitRepository::open("/tmp/test");
        assert_eq!(repo.root(), Path::new("/tmp/test"));
    }

    // -- Real git integration tests (use temp dirs) -------------------------

    /// Helper: create a temp dir with `git init`.
    fn init_temp_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create temp dir");
        Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("git init");
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir.path())
            .output()
            .expect("git config email");
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir.path())
            .output()
            .expect("git config name");
        dir
    }

    #[test]
    fn real_git_branch() {
        let dir = init_temp_repo();
        let repo = GitRepository::open(dir.path());
        // Create an initial commit so HEAD exists.
        std::fs::write(dir.path().join("init.txt"), "init").unwrap();
        repo.git_stage(Path::new("init.txt")).unwrap();
        repo.git_commit("initial").unwrap();

        let branch = repo.git_branch().unwrap();
        // Default branch is typically "main" or "master".
        assert!(!branch.is_empty());
    }

    #[test]
    fn real_git_status_untracked() {
        let dir = init_temp_repo();
        let repo = GitRepository::open(dir.path());
        std::fs::write(dir.path().join("hello.txt"), "hello").unwrap();

        let entries = repo.git_status().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index_status, FileStatus::Untracked);
        assert_eq!(entries[0].path, PathBuf::from("hello.txt"));
    }

    #[test]
    fn real_git_stage_and_commit() {
        let dir = init_temp_repo();
        let repo = GitRepository::open(dir.path());

        std::fs::write(dir.path().join("file.txt"), "content").unwrap();
        repo.git_stage(Path::new("file.txt")).unwrap();

        // Should now appear as staged (Added).
        let entries = repo.git_status().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index_status, FileStatus::Added);

        // Commit and verify clean status.
        repo.git_commit("add file").unwrap();
        let entries = repo.git_status().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn real_git_unstage() {
        let dir = init_temp_repo();
        let repo = GitRepository::open(dir.path());

        // Need initial commit first.
        std::fs::write(dir.path().join("base.txt"), "v1").unwrap();
        repo.git_stage(Path::new("base.txt")).unwrap();
        repo.git_commit("init").unwrap();

        // Modify and stage.
        std::fs::write(dir.path().join("base.txt"), "v2").unwrap();
        repo.git_stage(Path::new("base.txt")).unwrap();

        // Unstage it.
        repo.git_unstage(Path::new("base.txt")).unwrap();

        let entries = repo.git_status().unwrap();
        assert_eq!(entries.len(), 1);
        // Should be modified in worktree only.
        assert_eq!(entries[0].worktree_status, FileStatus::Modified);
    }

    #[test]
    fn real_git_discard() {
        let dir = init_temp_repo();
        let repo = GitRepository::open(dir.path());

        std::fs::write(dir.path().join("f.txt"), "original").unwrap();
        repo.git_stage(Path::new("f.txt")).unwrap();
        repo.git_commit("init").unwrap();

        // Modify the file.
        std::fs::write(dir.path().join("f.txt"), "changed").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
            "changed"
        );

        // Discard changes.
        repo.git_discard(Path::new("f.txt")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
            "original"
        );
    }

    #[test]
    fn real_git_diff() {
        let dir = init_temp_repo();
        let repo = GitRepository::open(dir.path());

        std::fs::write(dir.path().join("d.txt"), "line1\nline2\n").unwrap();
        repo.git_stage(Path::new("d.txt")).unwrap();
        repo.git_commit("init").unwrap();

        std::fs::write(dir.path().join("d.txt"), "line1\nmodified\n").unwrap();

        let hunks = repo.git_diff(Path::new("d.txt")).unwrap();
        assert!(!hunks.is_empty());
        let has_removed = hunks[0].lines.iter().any(|l| matches!(l, DiffLine::Removed(_)));
        let has_added = hunks[0].lines.iter().any(|l| matches!(l, DiffLine::Added(_)));
        assert!(has_removed);
        assert!(has_added);
    }

    #[test]
    fn real_git_diff_staged() {
        let dir = init_temp_repo();
        let repo = GitRepository::open(dir.path());

        std::fs::write(dir.path().join("s.txt"), "v1\n").unwrap();
        repo.git_stage(Path::new("s.txt")).unwrap();
        repo.git_commit("init").unwrap();

        std::fs::write(dir.path().join("s.txt"), "v2\n").unwrap();
        repo.git_stage(Path::new("s.txt")).unwrap();

        let hunks = repo.git_diff_staged(Path::new("s.txt")).unwrap();
        assert!(!hunks.is_empty());
    }

    #[test]
    fn real_git_log() {
        let dir = init_temp_repo();
        let repo = GitRepository::open(dir.path());

        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        repo.git_stage(Path::new("a.txt")).unwrap();
        repo.git_commit("first commit").unwrap();

        std::fs::write(dir.path().join("b.txt"), "b").unwrap();
        repo.git_stage(Path::new("b.txt")).unwrap();
        repo.git_commit("second commit").unwrap();

        let log = repo.git_log(5).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].message, "second commit");
        assert_eq!(log[1].message, "first commit");
    }

    #[test]
    fn real_git_grouped_status() {
        let dir = init_temp_repo();
        let repo = GitRepository::open(dir.path());

        // Create initial commit.
        std::fs::write(dir.path().join("tracked.txt"), "v1").unwrap();
        repo.git_stage(Path::new("tracked.txt")).unwrap();
        repo.git_commit("init").unwrap();

        // Modify tracked file (unstaged).
        std::fs::write(dir.path().join("tracked.txt"), "v2").unwrap();
        // Create untracked file.
        std::fs::write(dir.path().join("new.txt"), "new").unwrap();
        // Stage a new file.
        std::fs::write(dir.path().join("staged.txt"), "staged").unwrap();
        repo.git_stage(Path::new("staged.txt")).unwrap();

        let (staged, unstaged, untracked) = repo.grouped_status().unwrap();
        assert!(!staged.is_empty(), "should have staged files");
        assert!(!unstaged.is_empty(), "should have unstaged files");
        assert!(!untracked.is_empty(), "should have untracked files");
    }
}
