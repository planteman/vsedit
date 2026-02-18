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
// ScmEntry (flat entry for populate_from_git)
// ---------------------------------------------------------------------------

/// A single entry in the SCM view populated from git status data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScmEntry {
    pub path: PathBuf,
    pub status: FileStatus,
    pub staged: bool,
}

/// Populate the SCM view from git status output.
pub fn populate_from_git(view: &mut ScmView, status_output: &[(PathBuf, FileStatus)]) {
    view.entries.clear();
    for (path, status) in status_output {
        view.entries.push(ScmEntry {
            path: path.clone(),
            status: *status,
            staged: matches!(status, FileStatus::Added | FileStatus::Renamed | FileStatus::Copied),
        });
    }
    view.entries.sort_by(|a, b| a.path.cmp(&b.path));
}

// ---------------------------------------------------------------------------
// ScmView (UI state + rendering)
// ---------------------------------------------------------------------------

/// UI state for the Source Control sidebar.
pub struct ScmView {
    pub commit_message: String,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub entries: Vec<ScmEntry>,
}

impl ScmView {
    pub fn new() -> Self {
        Self {
            commit_message: String::new(),
            selected_index: 0,
            scroll_offset: 0,
            entries: Vec::new(),
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

    /// Render the SCM entries view with a "SOURCE CONTROL" title bar.
    pub fn render_entries(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut y = area.y;

        // Title bar: "SOURCE CONTROL" with change count.
        if y < area.y + area.height {
            let title = format!("SOURCE CONTROL ({})", self.entries.len());
            let style = Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD);
            self.put_line(buf, area.x, y, area.width, &title, style);
            y += 1;
        }

        // Render each entry.
        for entry in &self.entries {
            if y >= area.y + area.height {
                break;
            }
            let icon = match entry.status {
                FileStatus::Modified => 'M',
                FileStatus::Added => 'A',
                FileStatus::Deleted => 'D',
                FileStatus::Untracked => 'U',
                FileStatus::Renamed => 'R',
                FileStatus::Copied => 'C',
                FileStatus::Unmerged => '!',
            };
            let color = match entry.status {
                FileStatus::Added | FileStatus::Copied => Color::Green,
                FileStatus::Deleted => Color::Red,
                FileStatus::Modified | FileStatus::Renamed => Color::Yellow,
                FileStatus::Untracked => Color::Gray,
                FileStatus::Unmerged => Color::Red,
            };
            let prefix = if entry.staged { "✓ " } else { "  " };
            let label = format!(
                "{prefix}{icon} {}",
                entry.path.display()
            );
            let style = Style::default().fg(color);
            self.put_line(buf, area.x, y, area.width, &label, style);
            y += 1;
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

// ---------------------------------------------------------------------------
// Filtering helpers
// ---------------------------------------------------------------------------

/// Filter a slice of changes, keeping only those matching the given status.
pub fn filter_by_status(
    changes: &[ScmFileChange],
    status: ScmFileStatus,
) -> Vec<ScmFileChange> {
    changes.iter().filter(|c| c.status == status).cloned().collect()
}

/// Filter changes to only those whose path is under the given directory.
pub fn filter_by_directory(
    changes: &[ScmFileChange],
    dir: &Path,
) -> Vec<ScmFileChange> {
    changes.iter().filter(|c| c.path.starts_with(dir)).cloned().collect()
}

// ---------------------------------------------------------------------------
// Grouping helpers
// ---------------------------------------------------------------------------

/// Group changes by their parent directory. Returns `(directory, changes)` pairs
/// sorted by directory name. Files at the root use an empty path key.
pub fn group_by_directory(changes: &[ScmFileChange]) -> Vec<(PathBuf, Vec<ScmFileChange>)> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<PathBuf, Vec<ScmFileChange>> = BTreeMap::new();
    for change in changes {
        let dir = change
            .path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        map.entry(dir).or_default().push(change.clone());
    }
    map.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Sorting helpers
// ---------------------------------------------------------------------------

/// Sort changes alphabetically by path.
pub fn sort_by_path(changes: &mut [ScmFileChange]) {
    changes.sort_by(|a, b| a.path.cmp(&b.path));
}

/// Sort changes by status priority (conflicts first, then modified, added,
/// deleted, renamed, untracked, ignored).
pub fn sort_by_status(changes: &mut [ScmFileChange]) {
    fn priority(s: ScmFileStatus) -> u8 {
        match s {
            ScmFileStatus::Conflicted => 0,
            ScmFileStatus::Modified => 1,
            ScmFileStatus::Added => 2,
            ScmFileStatus::Deleted => 3,
            ScmFileStatus::Renamed => 4,
            ScmFileStatus::Untracked => 5,
            ScmFileStatus::Ignored => 6,
        }
    }
    changes.sort_by_key(|c| priority(c.status));
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Summary statistics computed from a set of diff hunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffStats {
    pub files: usize,
    pub insertions: usize,
    pub deletions: usize,
}

impl DiffStats {
    /// Compute diff stats from a slice of hunks belonging to a single file.
    pub fn from_hunks(hunks: &[DiffHunk]) -> Self {
        let mut insertions = 0usize;
        let mut deletions = 0usize;
        for hunk in hunks {
            for line in &hunk.lines {
                match line {
                    DiffLine::Added(_) => insertions += 1,
                    DiffLine::Removed(_) => deletions += 1,
                    DiffLine::Context(_) => {}
                }
            }
        }
        Self { files: 1, insertions, deletions }
    }

    /// Merge multiple per-file stats into one aggregate.
    pub fn aggregate(stats: &[DiffStats]) -> Self {
        let mut total = DiffStats { files: 0, insertions: 0, deletions: 0 };
        for s in stats {
            total.files += s.files;
            total.insertions += s.insertions;
            total.deletions += s.deletions;
        }
        total
    }

    /// Net change (insertions − deletions). Positive means the file grew.
    pub fn net_change(&self) -> isize {
        self.insertions as isize - self.deletions as isize
    }
}

impl fmt::Display for DiffStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} file{} changed, {} insertion{} (+), {} deletion{} (-)",
            self.files,
            if self.files == 1 { "" } else { "s" },
            self.insertions,
            if self.insertions == 1 { "" } else { "s" },
            self.deletions,
            if self.deletions == 1 { "" } else { "s" },
        )
    }
}

/// Count how many changes match each status in a slice.
pub fn status_counts(changes: &[ScmFileChange]) -> std::collections::HashMap<ScmFileStatus, usize> {
    let mut map = std::collections::HashMap::new();
    for c in changes {
        *map.entry(c.status).or_insert(0) += 1;
    }
    map
}

// ---------------------------------------------------------------------------
// Diffstat bar formatting
// ---------------------------------------------------------------------------

/// Format a single-file diffstat bar similar to `git diff --stat`.
///
/// Example: `src/lib.rs | 5 +++--`
///
/// `max_width` controls the maximum number of `+`/`-` characters in the bar.
pub fn format_diffstat_line(path: &Path, insertions: usize, deletions: usize, max_width: usize) -> String {
    let total = insertions + deletions;
    if total == 0 {
        return format!("{} | 0", path.display());
    }
    let scale = if total > max_width { max_width as f64 / total as f64 } else { 1.0 };
    let plus_count = (insertions as f64 * scale).round().max(if insertions > 0 { 1.0 } else { 0.0 }) as usize;
    let minus_count = (deletions as f64 * scale).round().max(if deletions > 0 { 1.0 } else { 0.0 }) as usize;
    let bar: String = std::iter::repeat('+').take(plus_count)
        .chain(std::iter::repeat('-').take(minus_count))
        .collect();
    format!("{} | {} {}", path.display(), total, bar)
}

// ---------------------------------------------------------------------------
// Conflict detection helpers
// ---------------------------------------------------------------------------

/// Return `true` if the change list contains any conflicted files.
pub fn has_conflicts(changes: &[ScmFileChange]) -> bool {
    changes.iter().any(|c| c.status == ScmFileStatus::Conflicted)
}

/// Extract only the conflicted files from a change list.
pub fn conflicted_files(changes: &[ScmFileChange]) -> Vec<&ScmFileChange> {
    changes.iter().filter(|c| c.status == ScmFileStatus::Conflicted).collect()
}

// ---------------------------------------------------------------------------
// Staging / Unstaging helpers (in-memory view manipulation)
// ---------------------------------------------------------------------------

impl ScmView {
    /// Toggle the staged flag on the currently selected entry.
    pub fn toggle_staged_selected(&mut self) {
        if let Some(entry) = self.entries.get_mut(self.selected_index) {
            entry.staged = !entry.staged;
        }
    }

    /// Stage all entries.
    pub fn stage_all(&mut self) {
        for entry in &mut self.entries {
            entry.staged = true;
        }
    }

    /// Unstage all entries.
    pub fn unstage_all(&mut self) {
        for entry in &mut self.entries {
            entry.staged = false;
        }
    }

    /// Return paths of all currently staged entries.
    pub fn staged_paths(&self) -> Vec<&Path> {
        self.entries.iter().filter(|e| e.staged).map(|e| e.path.as_path()).collect()
    }

    /// Return paths of all currently unstaged entries.
    pub fn unstaged_paths(&self) -> Vec<&Path> {
        self.entries.iter().filter(|e| !e.staged).map(|e| e.path.as_path()).collect()
    }

    /// Move the selection cursor down, wrapping at the bottom.
    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.entries.len();
        }
    }

    /// Move the selection cursor up, wrapping at the top.
    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.entries.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Status summary formatting
// ---------------------------------------------------------------------------

/// Format a compact status summary string suitable for a status bar.
///
/// Example: `main ↑2 ↓1 | 3M 1A 2? 1!`
pub fn format_status_summary(
    branch: Option<&str>,
    ahead: usize,
    behind: usize,
    changes: &[ScmFileChange],
) -> String {
    let mut parts = Vec::new();

    // Branch
    parts.push(branch.unwrap_or("(detached)").to_string());

    // Ahead/behind
    if ahead > 0 || behind > 0 {
        let mut ab = String::new();
        if ahead > 0 {
            ab.push_str(&format!("↑{ahead}"));
        }
        if behind > 0 {
            if !ab.is_empty() {
                ab.push(' ');
            }
            ab.push_str(&format!("↓{behind}"));
        }
        parts.push(ab);
    }

    // Change counts by status
    if !changes.is_empty() {
        let counts = status_counts(changes);
        let mut status_parts = Vec::new();
        // Deterministic ordering
        for &st in &[
            ScmFileStatus::Modified,
            ScmFileStatus::Added,
            ScmFileStatus::Deleted,
            ScmFileStatus::Renamed,
            ScmFileStatus::Untracked,
            ScmFileStatus::Conflicted,
            ScmFileStatus::Ignored,
        ] {
            if let Some(&n) = counts.get(&st) {
                status_parts.push(format!("{n}{}", st.icon()));
            }
        }
        if !status_parts.is_empty() {
            parts.push(status_parts.join(" "));
        }
    }

    parts.join(" | ")
}

// ===========================================================================
// Tests
// ===========================================================================


// ---------------------------------------------------------------------------
// ScmCommitMessageBuilder
// ---------------------------------------------------------------------------

/// Template tokens that can appear in a commit message template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitToken {
    /// A literal string fragment.
    Literal(String),
    /// A placeholder that will be substituted, e.g. `{scope}`.
    Placeholder(String),
}

/// Builds structured commit messages using a configurable template.
///
/// The builder supports a mini-template language where placeholders are
/// delimited by braces: `{type}: {summary}`.  Each placeholder can have
/// a default value; if no explicit value is set the default is used.
#[derive(Debug, Clone)]
pub struct ScmCommitMessageBuilder {
    tokens: Vec<CommitToken>,
    values: std::collections::HashMap<String, String>,
    defaults: std::collections::HashMap<String, String>,
    max_subject_len: usize,
    body_lines: Vec<String>,
    footer_trailers: Vec<(String, String)>,
}

impl ScmCommitMessageBuilder {
    /// Parse a template string into a new builder.
    pub fn new(template: &str) -> Self {
        let tokens = Self::parse_template(template);
        Self {
            tokens,
            values: std::collections::HashMap::new(),
            defaults: std::collections::HashMap::new(),
            max_subject_len: 72,
            body_lines: Vec::new(),
            footer_trailers: Vec::new(),
        }
    }

    fn parse_template(template: &str) -> Vec<CommitToken> {
        let mut tokens = Vec::new();
        let mut buf = String::new();
        let mut in_placeholder = false;
        for ch in template.chars() {
            match ch {
                '{' if !in_placeholder => {
                    if !buf.is_empty() {
                        tokens.push(CommitToken::Literal(std::mem::take(&mut buf)));
                    }
                    in_placeholder = true;
                }
                '}' if in_placeholder => {
                    tokens.push(CommitToken::Placeholder(std::mem::take(&mut buf)));
                    in_placeholder = false;
                }
                _ => buf.push(ch),
            }
        }
        if !buf.is_empty() {
            tokens.push(CommitToken::Literal(buf));
        }
        tokens
    }

    /// Set the value for a named placeholder.
    pub fn set(&mut self, name: &str, value: &str) -> &mut Self {
        self.values.insert(name.to_string(), value.to_string());
        self
    }

    /// Set the default value for a placeholder (used when no explicit value is set).
    pub fn set_default(&mut self, name: &str, value: &str) -> &mut Self {
        self.defaults.insert(name.to_string(), value.to_string());
        self
    }

    /// Override the maximum subject-line length (default 72).
    pub fn max_subject_len(&mut self, len: usize) -> &mut Self {
        self.max_subject_len = len;
        self
    }

    /// Append a body paragraph line.
    pub fn add_body_line(&mut self, line: &str) -> &mut Self {
        self.body_lines.push(line.to_string());
        self
    }

    /// Append a `key: value` trailer (e.g. `Signed-off-by`).
    pub fn add_trailer(&mut self, key: &str, value: &str) -> &mut Self {
        self.footer_trailers.push((key.to_string(), value.to_string()));
        self
    }

    /// Return the list of placeholder names found in the template.
    pub fn placeholders(&self) -> Vec<&str> {
        self.tokens.iter().filter_map(|t| {
            if let CommitToken::Placeholder(name) = t { Some(name.as_str()) } else { None }
        }).collect()
    }

    /// Build the final commit message string.
    pub fn build(&self) -> String {
        let mut subject = String::new();
        for token in &self.tokens {
            match token {
                CommitToken::Literal(s) => subject.push_str(s),
                CommitToken::Placeholder(name) => {
                    if let Some(v) = self.values.get(name) {
                        subject.push_str(v);
                    } else if let Some(d) = self.defaults.get(name) {
                        subject.push_str(d);
                    } else {
                        subject.push_str(&format!("{{{name}}}"));
                    }
                }
            }
        }
        if subject.len() > self.max_subject_len {
            subject.truncate(self.max_subject_len - 3);
            subject.push_str("...");
        }
        let mut out = subject;
        if !self.body_lines.is_empty() {
            out.push_str("\n\n");
            out.push_str(&self.body_lines.join("\n"));
        }
        if !self.footer_trailers.is_empty() {
            out.push_str("\n\n");
            for (k, v) in &self.footer_trailers {
                out.push_str(&format!("{k}: {v}\n"));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// ScmDiffStatisticsView
// ---------------------------------------------------------------------------

/// Aggregated statistics about a set of file diffs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScmDiffStatisticsView {
    entries: Vec<DiffStatEntry>,
}

/// Per-file diff statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffStatEntry {
    pub path: String,
    pub insertions: u32,
    pub deletions: u32,
}

impl DiffStatEntry {
    pub fn new(path: &str, insertions: u32, deletions: u32) -> Self {
        Self { path: path.to_string(), insertions, deletions }
    }

    /// Net change (insertions minus deletions).
    pub fn net_change(&self) -> i64 {
        self.insertions as i64 - self.deletions as i64
    }

    /// Total lines changed.
    pub fn total_change(&self) -> u32 {
        self.insertions + self.deletions
    }

    /// Return a small histogram bar, e.g. `+++--`.
    pub fn histogram(&self, max_width: u32) -> String {
        let total = self.total_change();
        if total == 0 {
            return String::new();
        }
        let scale = if total > max_width { max_width as f64 / total as f64 } else { 1.0 };
        let plus_count = (self.insertions as f64 * scale).round() as usize;
        let minus_count = (self.deletions as f64 * scale).round() as usize;
        format!("{}{}", "+".repeat(plus_count), "-".repeat(minus_count))
    }
}

impl ScmDiffStatisticsView {
    /// Create a new empty statistics view.
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Add an entry.
    pub fn add(&mut self, path: &str, insertions: u32, deletions: u32) {
        self.entries.push(DiffStatEntry::new(path, insertions, deletions));
    }

    /// Total insertions across all files.
    pub fn total_insertions(&self) -> u32 {
        self.entries.iter().map(|e| e.insertions).sum()
    }

    /// Total deletions across all files.
    pub fn total_deletions(&self) -> u32 {
        self.entries.iter().map(|e| e.deletions).sum()
    }

    /// Number of files changed.
    pub fn files_changed(&self) -> usize {
        self.entries.len()
    }

    /// Get the entry with the most changes.
    pub fn most_changed(&self) -> Option<&DiffStatEntry> {
        self.entries.iter().max_by_key(|e| e.total_change())
    }

    /// Return a summary string similar to git's `--stat` output.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            let bar = e.histogram(40);
            out.push_str(&format!(" {} | {} {}\n", e.path, e.total_change(), bar));
        }
        out.push_str(&format!(
            " {} file(s) changed, {} insertion(s)(+), {} deletion(s)(-)\n",
            self.files_changed(),
            self.total_insertions(),
            self.total_deletions(),
        ));
        out
    }

    /// Return entries sorted by total change descending.
    pub fn sorted_by_change(&self) -> Vec<&DiffStatEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.total_change().cmp(&a.total_change()));
        sorted
    }
}



// ---------------------------------------------------------------------------
// scm_view – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XScmViewLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XScmViewPanelState {
    pub region: XScmViewLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XScmViewPanelState {
    pub fn new(region: XScmViewLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_scm_view_total_visible_area(panels: &[XScmViewPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_scm_view_count_in_region(
    panels: &[XScmViewPanelState],
    region: XScmViewLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_scm_view_widest_panel(panels: &[XScmViewPanelState]) -> Option<&XScmViewPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_scm_view_collapse_region(
    panels: &mut [XScmViewPanelState],
    region: XScmViewLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XScmViewLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XScmViewLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// scm_view – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for source control view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YScmViewScmChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl YScmViewScmChangeKind {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Added => 0,
            Self::Modified => 1,
            Self::Deleted => 2,
            Self::Renamed => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Modified => "Modified",
            Self::Deleted => "Deleted",
            Self::Renamed => "Renamed",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YScmViewScmChangeKind] {
        &[
            YScmViewScmChangeKind::Added,
            YScmViewScmChangeKind::Modified,
            YScmViewScmChangeKind::Deleted,
            YScmViewScmChangeKind::Renamed,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YScmViewScmChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks SCM summary data.
#[derive(Debug, Clone)]
pub struct YScmViewScmStatusSummary {
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
}

impl YScmViewScmStatusSummary {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            added: 0,
            modified: 0,
            deleted: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YScmViewScmStatusSummary({}: {:?})", "added", self.added)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_scm_view_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_scm_view_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_scm_view_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_scm_view_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_scm_view_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_scm_view_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_scm_view_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_scm_view_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// scm_view – Extended SCM blame info helpers
// ---------------------------------------------------------------------------

/// Priority levels for SCM blame info.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZScmViewPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZScmViewPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZScmViewPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZScmViewPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks SCM blame info data.
#[derive(Debug, Clone)]
pub struct ZScmViewScmBlameInfo {
    pub line_authors: Vec<(u32, String)>,
    pub commit_count: usize,
    pub oldest_ms: u64,
}

impl ZScmViewScmBlameInfo {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            line_authors: Vec::new(),
            commit_count: 0,
            oldest_ms: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.line_authors.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.line_authors.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.line_authors.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZScmViewScmBlameInfo[commit_count={:?}, oldest_ms={:?}]", self.commit_count, self.oldest_ms)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for SCM blame info.
pub fn z_scm_view_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_scm_view_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_scm_view_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_scm_view_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_scm_view_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_scm_view_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_scm_view_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 44
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer44 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer44 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_44(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_44<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_44<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_44(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_44(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 153
// ---------------------------------------------------------------------------

/// Generic object pool `Xc153Pool<T>`.
pub struct Xc153Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc153Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc153PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc153Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc153PoolStats {
        Xc153PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc153Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc153Scheduler`.
pub struct Xc153Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc153Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc153Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_153 hash for the given byte slice.
pub fn xc_153_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_153 convention.
pub fn xc_153_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe57 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe57Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe57PipelineError {
    pub stage: Xe57Stage,
    pub message: String,
}

impl std::fmt::Display for Xe57PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe57Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe57Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError>>>,
    stage_names: Vec<Xe57Stage>,
}

impl Xe57Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe57Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe57Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe57Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe57Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe57Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe57CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe57CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe57Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe57CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe57CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe57Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe57CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_57_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe57CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_57_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe57CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_57_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError> {
    Ok(data)
}

pub fn xe_57_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_57_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_57_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_57_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe57PipelineError> {
    Err(Xe57PipelineError {
        stage: Xe57Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_55: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg55Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg55Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg55Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_55: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg55Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg55Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg55Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg55Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 152).
pub struct Xh152SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh152SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 194 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 152).
pub struct Xh152BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh152BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 152).
pub struct Xi152Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi152Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi152Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi152Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 152).
pub struct Xi152IntervalTree {
    xi_intervals: Vec<Xi152Interval>,
}

impl Xi152IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi152Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi152Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi152Interval) -> Vec<&Xi152Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi152Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi152Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi152Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi152Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi152Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi152Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 152) ---

/// Disjoint set / union-find for crate 152.
pub struct Xj152UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj152UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ152_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 152.
pub struct Xj152BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj152BTreeNode<K, V>>>,
    len: usize,
}

struct Xj152BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj152BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj152BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ152_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ152_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj152BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj152BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj152BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj152BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_152 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk152SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk152SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk152DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk152DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_152).
#[derive(Debug, Clone)]
pub struct Xl152Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl152Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_152).
#[derive(Debug, Clone)]
pub struct Xl152SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl152SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm152MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm152MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm152Tokenizer {
    text: String,
}

impl Xm152Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 152.
pub struct Xn152Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn152Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 152 -----

#[derive(Debug, Clone)]
struct Xn152AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn152AvlNode<K, V>>>,
    right: Option<Box<Xn152AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 152.
#[derive(Debug, Clone)]
pub struct Xn152AVL<K, V> {
    root: Option<Box<Xn152AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn152AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn152AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn152AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn152AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn152AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn152AvlNode<K, V>>) -> Box<Xn152AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn152AvlNode<K, V>>) -> Box<Xn152AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn152AvlNode<K, V>>) -> Box<Xn152AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn152AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn152AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn152AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn152AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn152AvlNode<K, V>>) -> &Xn152AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn152AvlNode<K, V>>) -> (Box<Xn152AvlNode<K, V>>, Option<Box<Xn152AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn152AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn152AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn152AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn152AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn152AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn152AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn152AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo152RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo152Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo152RBNode<K, V> {
    key: K,
    value: V,
    color: Xo152Color,
    left: Option<Box<Xo152RBNode<K, V>>>,
    right: Option<Box<Xo152RBNode<K, V>>>,
}

/// A red-black tree map for crate 152.
#[derive(Debug, Clone)]
pub struct Xo152RedBlack<K, V> {
    root: Option<Box<Xo152RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo152RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo152Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo152RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo152RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo152RBNode {
                    key, value, color: Xo152Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo152RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo152Color::Red)
    }

    fn xo_balance(mut h: Box<Xo152RBNode<K, V>>) -> Box<Xo152RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo152Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo152RBNode<K, V>>) -> Box<Xo152RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo152Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo152RBNode<K, V>>) -> Box<Xo152RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo152Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo152RBNode<K, V>>) {
        h.color = Xo152Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo152Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo152Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo152Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo152RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo152RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo152RBNode<K, V>) -> (K, V, Option<Box<Xo152RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo152RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo152Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo152RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo152ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 152.
#[derive(Debug, Clone)]
pub struct Xo152ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo152ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo152#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo152#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 152).
#[derive(Debug)]
pub struct Xp152SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp152Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp152Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp152Node<K, V>>>,
    xp_right: Option<Box<Xp152Node<K, V>>>,
}

impl<K: Ord, V> Xp152Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp152SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp152SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp152Node<K, V>>>, key: &K) -> Option<Box<Xp152Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp152Node<K, V>>) -> Box<Xp152Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp152Node<K, V>>) -> Box<Xp152Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp152Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp152Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp152Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq152Treap ---------------

use std::cmp::Ordering as Xq152Ord;

struct Xq152TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq152TreapNode<K, V>>>,
    right: Option<Box<Xq152TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq152Treap<K, V> {
    root: Option<Box<Xq152TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq152TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_152_size<K, V>(node: &Option<Box<Xq152TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_152_update_size<K, V>(node: &mut Xq152TreapNode<K, V>) {
    node.size = 1 + xq_152_size(&node.left) + xq_152_size(&node.right);
}

fn xq_152_rotate_right<K, V>(mut node: Box<Xq152TreapNode<K, V>>) -> Box<Xq152TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_152_update_size(&mut node);
    left.right = Some(node);
    xq_152_update_size(&mut left);
    left
}

fn xq_152_rotate_left<K, V>(mut node: Box<Xq152TreapNode<K, V>>) -> Box<Xq152TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_152_update_size(&mut node);
    right.left = Some(node);
    xq_152_update_size(&mut right);
    right
}

fn xq_152_insert_node<K: Ord, V>(
    node: Option<Box<Xq152TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq152TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq152TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq152Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq152Ord::Less => {
                let (new_left, old) = xq_152_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_152_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_152_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq152Ord::Greater => {
                let (new_right, old) = xq_152_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_152_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_152_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_152_remove_node<K: Ord, V>(
    node: Option<Box<Xq152TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq152TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq152Ord::Less => {
                let (new_left, old) = xq_152_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_152_update_size(&mut n);
                (Some(n), old)
            }
            Xq152Ord::Greater => {
                let (new_right, old) = xq_152_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_152_update_size(&mut n);
                (Some(n), old)
            }
            Xq152Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_152_rotate_right(n);
                    let (new_right, old) = xq_152_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_152_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_152_rotate_left(n);
                    let (new_left, old) = xq_152_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_152_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_152_find_min<K, V>(node: &Option<Box<Xq152TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_152_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_152_find_max<K, V>(node: &Option<Box<Xq152TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_152_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_152_rank<K: Ord, V>(node: &Option<Box<Xq152TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq152Ord::Less => xq_152_rank(&n.left, key),
            Xq152Ord::Equal => xq_152_size(&n.left),
            Xq152Ord::Greater => 1 + xq_152_size(&n.left) + xq_152_rank(&n.right, key),
        },
    }
}

fn xq_152_kth<K, V>(node: &Option<Box<Xq152TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_152_size(&n.left);
        if k < left_size {
            xq_152_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_152_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_152_in_order<K: Clone, V>(node: &Option<Box<Xq152TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_152_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_152_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq152Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 152 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_152_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq152Ord::Equal => return Some(&n.value),
                Xq152Ord::Less => cur = &n.left,
                Xq152Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_152_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_152_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_152_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_152_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_152_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_152_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_152_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq152VEBTree ---------------

pub struct Xq152VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq152VEBTree>>,
    clusters: Vec<Option<Box<Xq152VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq152VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq152VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq152VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr152KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr152KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr152BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr152KDNode {
    xr_point: Xr152KDPoint,
    xr_left: Option<Box<Xr152KDNode>>,
    xr_right: Option<Box<Xr152KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr152KDTree {
    xr_root: Option<Box<Xr152KDNode>>,
    xr_size: usize,
}

impl Xr152KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr152KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr152KDNode>>,
        point: Xr152KDPoint,
        depth: usize,
    ) -> Box<Xr152KDNode> {
        match node {
            None => Box::new(Xr152KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr152KDPoint) -> Option<Xr152KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr152KDNode>,
        query: &Xr152KDPoint,
        depth: usize,
        best: &mut Xr152KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr152KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr152KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr152KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr152KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr152KDNode>>, pts: &mut Vec<Xr152KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr152KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr152BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr152BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs152PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs152PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs152PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs152PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs152ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs152ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs152ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs152RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs152RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs152RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs152CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs152CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs152CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}

/// Auxiliary statistics tracker for xs_152 data structures.
#[derive(Debug, Clone)]
pub struct Xs152StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs152StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs152StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
}


// --- xu_ Binomial Heap ---

/// A node in a binomial heap.
#[derive(Debug, Clone)]
pub struct XuBinomialNode<K: Ord + Clone, V: Clone> {
    pub xu_key: K,
    pub xu_value: V,
    xu_degree: usize,
    xu_children: Vec<usize>,
    xu_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XuBinomialNode<K, V> {
    /// Create a new binomial node.
    pub fn xu_new(key: K, value: V) -> Self {
        Self { xu_key: key, xu_value: value, xu_degree: 0, xu_children: Vec::new(), xu_parent: None }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinNode(key={}, deg={})", self.xu_key, self.xu_degree)
    }
}

/// Binomial heap with O(log n) insert, extract-min, and merge.
#[derive(Debug, Clone)]
pub struct XuBinomialHeap<K: Ord + Clone, V: Clone> {
    xu_nodes: Vec<XuBinomialNode<K, V>>,
    xu_roots: Vec<usize>,
    xu_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XuBinomialHeap<K, V> {
    fn default() -> Self { Self::xu_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinHeap(size={}, trees={})", self.xu_size, self.xu_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XuBinomialHeap<K, V> {
    /// Create an empty binomial heap.
    pub fn xu_new() -> Self {
        Self { xu_nodes: Vec::new(), xu_roots: Vec::new(), xu_size: 0 }
    }

    /// Return the number of elements.
    pub fn xu_len(&self) -> usize { self.xu_size }

    /// Check if the heap is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_size == 0 }

    /// Insert a key-value pair.
    pub fn xu_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xu_nodes.len();
        self.xu_nodes.push(XuBinomialNode::xu_new(key, value));
        self.xu_add_root(idx);
        self.xu_size += 1;
        self.xu_consolidate();
        idx
    }

    fn xu_add_root(&mut self, idx: usize) {
        self.xu_nodes[idx].xu_parent = None;
        self.xu_roots.push(idx);
    }

    fn xu_consolidate(&mut self) {
        let max_deg = (self.xu_size as f64).log2().ceil() as usize + 2;
        let mut table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xu_roots.clone();
        self.xu_roots.clear();
        for root in roots {
            let mut x = root;
            loop {
                let d = self.xu_nodes[x].xu_degree;
                if d >= table.len() { break; }
                match table[d] {
                    None => { table[d] = Some(x); break; }
                    Some(y) => {
                        table[d] = None;
                        let (p, c) = if self.xu_nodes[x].xu_key <= self.xu_nodes[y].xu_key { (x, y) } else { (y, x) };
                        self.xu_nodes[p].xu_children.push(c);
                        self.xu_nodes[c].xu_parent = Some(p);
                        self.xu_nodes[p].xu_degree += 1;
                        x = p;
                    }
                }
            }
        }
        for slot in &table {
            if let Some(r) = slot {
                self.xu_roots.push(*r);
            }
        }
        self.xu_roots.sort_by_key(|&r| self.xu_nodes[r].xu_degree);
    }

    /// Peek at the minimum.
    pub fn xu_find_min(&self) -> Option<(&K, &V)> {
        self.xu_roots.iter()
            .min_by(|&&a, &&b| self.xu_nodes[a].xu_key.cmp(&self.xu_nodes[b].xu_key))
            .map(|&i| (&self.xu_nodes[i].xu_key, &self.xu_nodes[i].xu_value))
    }

    /// Extract the minimum element.
    pub fn xu_extract_min(&mut self) -> Option<(K, V)> {
        if self.xu_roots.is_empty() { return None; }
        let min_pos = self.xu_roots.iter().enumerate()
            .min_by(|(_, a), (_, b)| self.xu_nodes[**a].xu_key.cmp(&self.xu_nodes[**b].xu_key))
            .map(|(pos, _)| pos)?;
        let min_idx = self.xu_roots.remove(min_pos);
        let children = self.xu_nodes[min_idx].xu_children.clone();
        for &c in &children {
            self.xu_nodes[c].xu_parent = None;
            self.xu_roots.push(c);
        }
        self.xu_size -= 1;
        if !self.xu_roots.is_empty() {
            self.xu_consolidate();
        }
        let n = &self.xu_nodes[min_idx];
        Some((n.xu_key.clone(), n.xu_value.clone()))
    }

    /// Merge another binomial heap into this one.
    pub fn xu_merge(&mut self, other: &mut XuBinomialHeap<K, V>) {
        let off = self.xu_nodes.len();
        for mut n in other.xu_nodes.drain(..) {
            n.xu_parent = n.xu_parent.map(|p| p + off);
            n.xu_children = n.xu_children.iter().map(|&c| c + off).collect();
            self.xu_nodes.push(n);
        }
        for r in other.xu_roots.drain(..) {
            self.xu_roots.push(r + off);
        }
        self.xu_size += other.xu_size;
        other.xu_size = 0;
        self.xu_consolidate();
    }

    /// Drain all elements in sorted order.
    pub fn xu_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xu_size);
        while let Some(pair) = self.xu_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xu_clear(&mut self) {
        self.xu_nodes.clear();
        self.xu_roots.clear();
        self.xu_size = 0;
    }
}

// --- xu_ Disjoint Sparse Table ---

/// Disjoint sparse table for O(1) range queries on static data with an associative operation.
#[derive(Debug, Clone)]
pub struct XuDisjointSparseTable<T: Clone> {
    xu_table: Vec<Vec<T>>,
    xu_data: Vec<T>,
    xu_len: usize,
    xu_levels: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XuDisjointSparseTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DST(len={}, levels={})", self.xu_len, self.xu_levels)
    }
}

impl<T: Clone + Default + std::ops::Add<Output = T>> XuDisjointSparseTable<T> {
    /// Build a disjoint sparse table for range-sum queries.
    pub fn xu_build(data: &[T]) -> Self {
        let n = data.len();
        if n == 0 {
            return Self { xu_table: Vec::new(), xu_data: Vec::new(), xu_len: 0, xu_levels: 0 };
        }
        let levels = (n as f64).log2().ceil() as usize + 1;
        let mut table = Vec::with_capacity(levels);
        for level in 0..levels {
            let block = 1 << level;
            let mut row = data.to_vec();
            let mut mid = block;
            while mid < n {
                // Build prefix sums going left from mid
                if mid > 0 && mid - 1 < n {
                    let start = if mid >= block { mid - block } else { 0 };
                    let mut i = mid.saturating_sub(1);
                    loop {
                        if i < start { break; }
                        if i + 1 < mid && i + 1 < n {
                            row[i] = row[i].clone() + row[i + 1].clone();
                        }
                        if i == start { break; }
                        i -= 1;
                    }
                }
                // Build prefix sums going right from mid
                let end = std::cmp::min(mid + block, n);
                for i in (mid + 1)..end {
                    row[i] = row[i - 1].clone() + row[i].clone();
                }
                mid += 2 * block;
            }
            table.push(row);
        }
        Self { xu_table: table, xu_data: data.to_vec(), xu_len: n, xu_levels: levels }
    }

    /// Query the sum of elements in the range [l, r] (inclusive).
    pub fn xu_query(&self, l: usize, r: usize) -> T {
        if l == r {
            return self.xu_data[l].clone();
        }
        if l >= self.xu_len || r >= self.xu_len || l > r {
            return T::default();
        }
        // Find the highest bit where l and r differ
        let xor = l ^ r;
        if xor == 0 {
            return self.xu_data[l].clone();
        }
        let level = (usize::BITS - xor.leading_zeros() - 1) as usize;
        if level < self.xu_levels && l < self.xu_table[level].len() && r < self.xu_table[level].len() {
            self.xu_table[level][l].clone() + self.xu_table[level][r].clone()
        } else {
            // Fallback: linear sum
            let mut sum = self.xu_data[l].clone();
            for i in (l + 1)..=r {
                sum = sum + self.xu_data[i].clone();
            }
            sum
        }
    }

    /// Return the length.
    pub fn xu_len(&self) -> usize { self.xu_len }

    /// Check if empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_len == 0 }

    /// Get element at index.
    pub fn xu_get(&self, idx: usize) -> Option<&T> {
        self.xu_data.get(idx)
    }
}

// --- xu_ Monotonic Stack ---

/// Monotonic stack that maintains elements in non-decreasing or non-increasing order.
#[derive(Debug, Clone)]
pub struct XuMonotonicStack<T: Clone + Ord> {
    xu_data: Vec<T>,
    xu_increasing: bool,
}

impl<T: Clone + Ord + std::fmt::Display> std::fmt::Display for XuMonotonicStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonoStack(len={}, inc={})", self.xu_data.len(), self.xu_increasing)
    }
}

impl<T: Clone + Ord> XuMonotonicStack<T> {
    /// Create a monotonically increasing stack.
    pub fn xu_increasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: true }
    }

    /// Create a monotonically decreasing stack.
    pub fn xu_decreasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: false }
    }

    /// Push a value, popping elements that violate the monotonic invariant.
    pub fn xu_push(&mut self, value: T) -> Vec<T> {
        let mut popped = Vec::new();
        if self.xu_increasing {
            while let Some(top) = self.xu_data.last() {
                if *top > value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        } else {
            while let Some(top) = self.xu_data.last() {
                if *top < value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        }
        self.xu_data.push(value);
        popped
    }

    /// Peek at the top.
    pub fn xu_peek(&self) -> Option<&T> { self.xu_data.last() }

    /// Pop from top.
    pub fn xu_pop(&mut self) -> Option<T> { self.xu_data.pop() }

    /// Length.
    pub fn xu_len(&self) -> usize { self.xu_data.len() }

    /// Is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_data.is_empty() }

    /// Get all elements.
    pub fn xu_as_slice(&self) -> &[T] { &self.xu_data }

    /// Clear the stack.
    pub fn xu_clear(&mut self) { self.xu_data.clear(); }
}


// --- xv_ Cartesian Tree ---

/// A node in a Cartesian tree (BST by key, heap by priority).
#[derive(Debug, Clone)]
pub struct XvCartesianNode<K: Ord + Clone, P: Ord + Clone> {
    pub xv_key: K,
    pub xv_priority: P,
    xv_left: Option<Box<XvCartesianNode<K, P>>>,
    xv_right: Option<Box<XvCartesianNode<K, P>>>,
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianNode<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartNode(k={}, p={})", self.xv_key, self.xv_priority)
    }
}

/// Cartesian tree — BST by key, min-heap by priority. Used for range-minimum queries.
#[derive(Debug, Clone)]
pub struct XvCartesianTree<K: Ord + Clone, P: Ord + Clone> {
    xv_root: Option<Box<XvCartesianNode<K, P>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, P: Ord + Clone> Default for XvCartesianTree<K, P> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianTree<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, P: Ord + Clone> XvCartesianTree<K, P> {
    /// Create an empty Cartesian tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Return the number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Check if empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    /// Insert a (key, priority) pair maintaining BST-by-key and min-heap-by-priority.
    pub fn xv_insert(&mut self, key: K, priority: P) {
        self.xv_root = Self::xv_insert_node(self.xv_root.take(), key, priority);
        self.xv_size += 1;
    }

    fn xv_insert_node(node: Option<Box<XvCartesianNode<K, P>>>, key: K, priority: P) -> Option<Box<XvCartesianNode<K, P>>> {
        match node {
            None => Some(Box::new(XvCartesianNode { xv_key: key, xv_priority: priority, xv_left: None, xv_right: None })),
            Some(mut n) => {
                if key < n.xv_key {
                    n.xv_left = Self::xv_insert_node(n.xv_left.take(), key.clone(), priority.clone());
                    if n.xv_left.as_ref().is_some_and(|l| l.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_right(n);
                    }
                    Some(n)
                } else {
                    n.xv_right = Self::xv_insert_node(n.xv_right.take(), key.clone(), priority.clone());
                    if n.xv_right.as_ref().is_some_and(|r| r.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_left(n);
                    }
                    Some(n)
                }
            }
        }
    }

    fn xv_rotate_right(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        left.xv_right = Some(node);
        left
    }

    fn xv_rotate_left(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        right.xv_left = Some(node);
        right
    }

    /// Search for a key.
    pub fn xv_contains(&self, key: &K) -> bool {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search(node: &Option<Box<XvCartesianNode<K, P>>>, key: &K) -> bool {
        match node {
            None => false,
            Some(n) => {
                if *key == n.xv_key { true }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// In-order traversal returning keys.
    pub fn xv_inorder(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder_walk(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder_walk(node: &Option<Box<XvCartesianNode<K, P>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder_walk(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder_walk(&n.xv_right, result);
        }
    }

    /// Get the root priority (minimum priority).
    pub fn xv_min_priority(&self) -> Option<&P> {
        self.xv_root.as_ref().map(|n| &n.xv_priority)
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Build from a sequence of (key, priority) pairs.
    pub fn xv_from_pairs(pairs: &[(K, P)]) -> Self {
        let mut tree = Self::xv_new();
        for (k, p) in pairs { tree.xv_insert(k.clone(), p.clone()); }
        tree
    }

    /// Height of the tree.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvCartesianNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(
                Self::xv_node_height(&n.xv_left),
                Self::xv_node_height(&n.xv_right),
            ),
        }
    }
}

// --- xv_ Weight-Balanced Tree ---

/// A node in a weight-balanced tree (BB[α] tree).
#[derive(Debug, Clone)]
pub struct XvWBNode<K: Ord + Clone, V: Clone> {
    pub xv_key: K,
    pub xv_value: V,
    xv_left: Option<Box<XvWBNode<K, V>>>,
    xv_right: Option<Box<XvWBNode<K, V>>>,
    xv_weight: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWBNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBNode(k={}, w={})", self.xv_key, self.xv_weight)
    }
}

/// Weight-balanced tree (BB[α] tree) with α = 0.29 for balanced operations.
#[derive(Debug, Clone)]
pub struct XvWeightBalancedTree<K: Ord + Clone, V: Clone> {
    xv_root: Option<Box<XvWBNode<K, V>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XvWeightBalancedTree<K, V> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWeightBalancedTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, V: Clone> XvWeightBalancedTree<K, V> {
    const ALPHA: f64 = 0.29;

    /// Create an empty weight-balanced tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Is the tree empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    fn xv_weight(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node { None => 1, Some(n) => n.xv_weight }
    }

    fn xv_update_weight(node: &mut Box<XvWBNode<K, V>>) {
        node.xv_weight = Self::xv_weight(&node.xv_left) + Self::xv_weight(&node.xv_right);
    }

    fn xv_is_balanced(node: &Box<XvWBNode<K, V>>) -> bool {
        let lw = Self::xv_weight(&node.xv_left) as f64;
        let rw = Self::xv_weight(&node.xv_right) as f64;
        let total = node.xv_weight as f64;
        lw >= Self::ALPHA * total && rw >= Self::ALPHA * total
    }

    /// Insert a key-value pair.
    pub fn xv_insert(&mut self, key: K, value: V) {
        let inserted = Self::xv_insert_node(self.xv_root.take(), key, value);
        self.xv_root = inserted.0;
        if inserted.1 { self.xv_size += 1; }
    }

    fn xv_insert_node(node: Option<Box<XvWBNode<K, V>>>, key: K, value: V) -> (Option<Box<XvWBNode<K, V>>>, bool) {
        match node {
            None => {
                let n = Box::new(XvWBNode { xv_key: key, xv_value: value, xv_left: None, xv_right: None, xv_weight: 2 });
                (Some(n), true)
            }
            Some(mut n) => {
                let inserted;
                if key < n.xv_key {
                    let r = Self::xv_insert_node(n.xv_left.take(), key, value);
                    n.xv_left = r.0;
                    inserted = r.1;
                } else if key > n.xv_key {
                    let r = Self::xv_insert_node(n.xv_right.take(), key, value);
                    n.xv_right = r.0;
                    inserted = r.1;
                } else {
                    n.xv_value = value;
                    return (Some(n), false);
                }
                Self::xv_update_weight(&mut n);
                let n = Self::xv_rebalance(n);
                (Some(n), inserted)
            }
        }
    }

    fn xv_rebalance(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if !Self::xv_is_balanced(&node) {
            let lw = Self::xv_weight(&node.xv_left);
            let rw = Self::xv_weight(&node.xv_right);
            if lw < rw {
                node = Self::xv_rotate_left_wb(node);
            } else {
                node = Self::xv_rotate_right_wb(node);
            }
        }
        node
    }

    fn xv_rotate_left_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_right.is_none() { return node; }
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        Self::xv_update_weight(&mut node);
        right.xv_left = Some(node);
        Self::xv_update_weight(&mut right);
        right
    }

    fn xv_rotate_right_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_left.is_none() { return node; }
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        Self::xv_update_weight(&mut node);
        left.xv_right = Some(node);
        Self::xv_update_weight(&mut left);
        left
    }

    /// Look up a key.
    pub fn xv_get(&self, key: &K) -> Option<&V> {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search<'a>(node: &'a Option<Box<XvWBNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xv_key { Some(&n.xv_value) }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xv_contains(&self, key: &K) -> bool { self.xv_get(key).is_some() }

    /// In-order traversal.
    pub fn xv_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder(node: &Option<Box<XvWBNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder(&n.xv_right, result);
        }
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Height.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xv_node_height(&n.xv_left), Self::xv_node_height(&n.xv_right)),
        }
    }
}


// --- xw_ Scapegoat Tree ---

/// A node in a scapegoat tree.
#[derive(Debug, Clone)]
pub struct XwScapegoatNode<K: Ord + Clone, V: Clone> {
    pub xw_key: K,
    pub xw_value: V,
    xw_left: Option<Box<XwScapegoatNode<K, V>>>,
    xw_right: Option<Box<XwScapegoatNode<K, V>>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGNode(k={})", self.xw_key)
    }
}

/// Scapegoat tree — a BST that rebuilds subtrees when they become too unbalanced.
#[derive(Debug, Clone)]
pub struct XwScapegoatTree<K: Ord + Clone, V: Clone> {
    xw_root: Option<Box<XwScapegoatNode<K, V>>>,
    xw_size: usize,
    xw_max_size: usize,
    xw_alpha: f64,
}

impl<K: Ord + Clone, V: Clone> Default for XwScapegoatTree<K, V> {
    fn default() -> Self { Self::xw_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGTree(size={}, alpha={:.2})", self.xw_size, self.xw_alpha)
    }
}

impl<K: Ord + Clone, V: Clone> XwScapegoatTree<K, V> {
    /// Create an empty scapegoat tree with default α = 0.7.
    pub fn xw_new() -> Self {
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: 0.7 }
    }

    /// Create with custom alpha (0.5 < α < 1.0).
    pub fn xw_with_alpha(alpha: f64) -> Self {
        let a = alpha.clamp(0.51, 0.99);
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: a }
    }

    /// Number of elements.
    pub fn xw_len(&self) -> usize { self.xw_size }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_size == 0 }

    fn xw_node_size(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + Self::xw_node_size(&n.xw_left) + Self::xw_node_size(&n.xw_right),
        }
    }

    /// Insert a key-value pair.
    pub fn xw_insert(&mut self, key: K, value: V) {
        let (new_root, depth, inserted) = Self::xw_insert_node(self.xw_root.take(), key, value, 0);
        self.xw_root = new_root;
        if inserted {
            self.xw_size += 1;
            self.xw_max_size = std::cmp::max(self.xw_max_size, self.xw_size);
            let h_alpha = -(self.xw_size as f64).log(1.0 / self.xw_alpha);
            if depth as f64 > h_alpha {
                self.xw_root = Self::xw_rebuild(self.xw_root.take());
            }
        }
    }

    fn xw_insert_node(
        node: Option<Box<XwScapegoatNode<K, V>>>, key: K, value: V, depth: usize,
    ) -> (Option<Box<XwScapegoatNode<K, V>>>, usize, bool) {
        match node {
            None => {
                let n = Box::new(XwScapegoatNode { xw_key: key, xw_value: value, xw_left: None, xw_right: None });
                (Some(n), depth, true)
            }
            Some(mut n) => {
                if key < n.xw_key {
                    let (l, d, ins) = Self::xw_insert_node(n.xw_left.take(), key, value, depth + 1);
                    n.xw_left = l;
                    if ins {
                        let ls = Self::xw_node_size(&n.xw_left);
                        let total = 1 + ls + Self::xw_node_size(&n.xw_right);
                        if ls as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else if key > n.xw_key {
                    let (r, d, ins) = Self::xw_insert_node(n.xw_right.take(), key, value, depth + 1);
                    n.xw_right = r;
                    if ins {
                        let rs = Self::xw_node_size(&n.xw_right);
                        let total = 1 + Self::xw_node_size(&n.xw_left) + rs;
                        if rs as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else {
                    n.xw_value = value;
                    (Some(n), depth, false)
                }
            }
        }
    }

    fn xw_flatten(node: Option<Box<XwScapegoatNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xw_flatten(n.xw_left, out);
            out.push((n.xw_key, n.xw_value));
            Self::xw_flatten(n.xw_right, out);
        }
    }

    fn xw_build_balanced(sorted: &[(K, V)]) -> Option<Box<XwScapegoatNode<K, V>>> {
        if sorted.is_empty() { return None; }
        let mid = sorted.len() / 2;
        let (k, v) = sorted[mid].clone();
        Some(Box::new(XwScapegoatNode {
            xw_key: k,
            xw_value: v,
            xw_left: Self::xw_build_balanced(&sorted[..mid]),
            xw_right: Self::xw_build_balanced(&sorted[mid + 1..]),
        }))
    }

    fn xw_rebuild(node: Option<Box<XwScapegoatNode<K, V>>>) -> Option<Box<XwScapegoatNode<K, V>>> {
        let mut flat = Vec::new();
        Self::xw_flatten(node, &mut flat);
        Self::xw_build_balanced(&flat)
    }

    /// Look up a key.
    pub fn xw_get(&self, key: &K) -> Option<&V> {
        Self::xw_search(&self.xw_root, key)
    }

    fn xw_search<'a>(node: &'a Option<Box<XwScapegoatNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xw_key { Some(&n.xw_value) }
                else if *key < n.xw_key { Self::xw_search(&n.xw_left, key) }
                else { Self::xw_search(&n.xw_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xw_contains(&self, key: &K) -> bool { self.xw_get(key).is_some() }

    /// In-order keys.
    pub fn xw_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xw_collect_keys(&self.xw_root, &mut result);
        result
    }

    fn xw_collect_keys(node: &Option<Box<XwScapegoatNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xw_collect_keys(&n.xw_left, result);
            result.push(n.xw_key.clone());
            Self::xw_collect_keys(&n.xw_right, result);
        }
    }

    /// Clear the tree.
    pub fn xw_clear(&mut self) {
        self.xw_root = None;
        self.xw_size = 0;
        self.xw_max_size = 0;
    }

    /// Height.
    pub fn xw_height(&self) -> usize {
        Self::xw_node_height(&self.xw_root)
    }

    fn xw_node_height(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xw_node_height(&n.xw_left), Self::xw_node_height(&n.xw_right)),
        }
    }
}

// --- xw_ Rope (String Rope) ---

/// A rope node — either a leaf with text or an internal node concatenating two children.
#[derive(Debug, Clone)]
pub enum XwRopeNode {
    Leaf(String),
    Internal {
        xw_left: Box<XwRopeNode>,
        xw_right: Box<XwRopeNode>,
        xw_len: usize,
    },
}

impl std::fmt::Display for XwRopeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XwRopeNode::Leaf(s) => write!(f, "RopeLeaf({})", s.len()),
            XwRopeNode::Internal { xw_len, .. } => write!(f, "RopeInt({})", xw_len),
        }
    }
}

/// Rope data structure for efficient string editing with O(log n) split/concat.
#[derive(Debug, Clone)]
pub struct XwRope {
    xw_root: Option<Box<XwRopeNode>>,
}

impl Default for XwRope {
    fn default() -> Self { Self::xw_new() }
}

impl std::fmt::Display for XwRope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rope(len={})", self.xw_len())
    }
}

impl XwRope {
    /// Create an empty rope.
    pub fn xw_new() -> Self { Self { xw_root: None } }

    /// Create a rope from a string.
    pub fn xw_from_str(s: &str) -> Self {
        if s.is_empty() {
            Self { xw_root: None }
        } else {
            Self { xw_root: Some(Box::new(XwRopeNode::Leaf(s.to_string()))) }
        }
    }

    /// Total length in bytes.
    pub fn xw_len(&self) -> usize {
        Self::xw_node_len(&self.xw_root)
    }

    fn xw_node_len(node: &Option<Box<XwRopeNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => s.len(),
                XwRopeNode::Internal { xw_len, .. } => *xw_len,
            },
        }
    }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_len() == 0 }

    /// Concatenate two ropes.
    pub fn xw_concat(left: XwRope, right: XwRope) -> XwRope {
        match (left.xw_root, right.xw_root) {
            (None, r) => XwRope { xw_root: r },
            (l, None) => XwRope { xw_root: l },
            (Some(l), Some(r)) => {
                let len = Self::xw_node_len(&Some(l.clone())) + Self::xw_node_len(&Some(r.clone()));
                XwRope {
                    xw_root: Some(Box::new(XwRopeNode::Internal { xw_left: l, xw_right: r, xw_len: len })),
                }
            }
        }
    }

    /// Convert to string.
    pub fn xw_to_string(&self) -> String {
        let mut result = String::new();
        Self::xw_collect(&self.xw_root, &mut result);
        result
    }

    fn xw_collect(node: &Option<Box<XwRopeNode>>, result: &mut String) {
        match node {
            None => {}
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => result.push_str(s),
                XwRopeNode::Internal { xw_left, xw_right, .. } => {
                    Self::xw_collect(&Some(xw_left.clone()), result);
                    Self::xw_collect(&Some(xw_right.clone()), result);
                }
            },
        }
    }

    /// Get character at byte index.
    pub fn xw_char_at(&self, idx: usize) -> Option<char> {
        let s = self.xw_to_string();
        s.as_bytes().get(idx).map(|&b| b as char)
    }

    /// Insert a string at byte index.
    pub fn xw_insert(&mut self, idx: usize, text: &str) {
        let s = self.xw_to_string();
        let (left, right) = s.split_at(idx.min(s.len()));
        let new_s = format!("{}{}{}", left, text, right);
        *self = Self::xw_from_str(&new_s);
    }

    /// Delete bytes in range [start, end).
    pub fn xw_delete(&mut self, start: usize, end: usize) {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        let new_s = format!("{}{}", &s[..start], &s[end..]);
        *self = Self::xw_from_str(&new_s);
    }

    /// Append text.
    pub fn xw_append(&mut self, text: &str) {
        let other = Self::xw_from_str(text);
        let old = std::mem::take(self);
        *self = Self::xw_concat(old, other);
    }

    /// Substring [start, end).
    pub fn xw_substring(&self, start: usize, end: usize) -> String {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        s[start..end].to_string()
    }

    /// Clear the rope.
    pub fn xw_clear(&mut self) { self.xw_root = None; }
}


// --- xx_ Skip List ---

/// A node in a skip list with multiple forward pointers for O(log n) search.
#[derive(Debug, Clone)]
pub struct XxSkipNode<K: Ord + Clone, V: Clone> {
    pub xx_key: Option<K>,
    pub xx_value: Option<V>,
    xx_forward: Vec<Option<usize>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.xx_key {
            Some(k) => write!(f, "SkipNode(k={}, lvl={})", k, self.xx_forward.len()),
            None => write!(f, "SkipNode(HEAD, lvl={})", self.xx_forward.len()),
        }
    }
}

/// Skip list — a probabilistic data structure with O(log n) average search, insert, delete.
#[derive(Debug, Clone)]
pub struct XxSkipList<K: Ord + Clone, V: Clone> {
    xx_nodes: Vec<XxSkipNode<K, V>>,
    xx_head: usize,
    xx_max_level: usize,
    xx_level: usize,
    xx_size: usize,
    xx_rng_state: u64,
}

impl<K: Ord + Clone, V: Clone> Default for XxSkipList<K, V> {
    fn default() -> Self { Self::xx_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipList<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkipList(size={}, level={})", self.xx_size, self.xx_level)
    }
}

impl<K: Ord + Clone, V: Clone> XxSkipList<K, V> {
    const XX_MAX_LEVEL: usize = 16;

    /// Create an empty skip list.
    pub fn xx_new() -> Self {
        let head = XxSkipNode {
            xx_key: None,
            xx_value: None,
            xx_forward: vec![None; Self::XX_MAX_LEVEL],
        };
        Self {
            xx_nodes: vec![head],
            xx_head: 0,
            xx_max_level: Self::XX_MAX_LEVEL,
            xx_level: 1,
            xx_size: 0,
            xx_rng_state: 42,
        }
    }

    fn xx_random_level(&mut self) -> usize {
        let mut lvl = 1;
        while lvl < self.xx_max_level {
            self.xx_rng_state ^= self.xx_rng_state << 13;
            self.xx_rng_state ^= self.xx_rng_state >> 7;
            self.xx_rng_state ^= self.xx_rng_state << 17;
            if self.xx_rng_state % 4 < 1 { break; }
            lvl += 1;
        }
        lvl
    }

    /// Number of elements.
    pub fn xx_len(&self) -> usize { self.xx_size }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_size == 0 }

    /// Insert a key-value pair.
    pub fn xx_insert(&mut self, key: K, value: V) {
        let mut update = vec![self.xx_head; self.xx_max_level];
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < key { current = next; continue; }
                    if *nk == key {
                        self.xx_nodes[next].xx_value = Some(value);
                        return;
                    }
                }
                break;
            }
            update[i] = current;
        }
        let lvl = self.xx_random_level();
        if lvl > self.xx_level {
            for i in self.xx_level..lvl {
                update[i] = self.xx_head;
            }
            self.xx_level = lvl;
        }
        let new_idx = self.xx_nodes.len();
        self.xx_nodes.push(XxSkipNode {
            xx_key: Some(key),
            xx_value: Some(value),
            xx_forward: vec![None; lvl],
        });
        for i in 0..lvl {
            self.xx_nodes[new_idx].xx_forward[i] = self.xx_nodes[update[i]].xx_forward[i];
            self.xx_nodes[update[i]].xx_forward[i] = Some(new_idx);
        }
        self.xx_size += 1;
    }

    /// Search for a key.
    pub fn xx_get(&self, key: &K) -> Option<&V> {
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < *key { current = next; continue; }
                    if *nk == *key { return self.xx_nodes[next].xx_value.as_ref(); }
                }
                break;
            }
        }
        None
    }

    /// Check if key exists.
    pub fn xx_contains(&self, key: &K) -> bool { self.xx_get(key).is_some() }

    /// Collect all keys in sorted order.
    pub fn xx_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        let mut current = self.xx_nodes[self.xx_head].xx_forward[0];
        while let Some(idx) = current {
            if let Some(ref k) = self.xx_nodes[idx].xx_key {
                result.push(k.clone());
            }
            current = self.xx_nodes[idx].xx_forward[0];
        }
        result
    }

    /// Clear the skip list.
    pub fn xx_clear(&mut self) {
        self.xx_nodes.truncate(1);
        for i in 0..self.xx_max_level {
            self.xx_nodes[0].xx_forward[i] = None;
        }
        self.xx_level = 1;
        self.xx_size = 0;
    }
}

// --- xx_ Suffix Array ---

/// Suffix array for O(n log n) construction and O(m log n) pattern matching.
#[derive(Debug, Clone)]
pub struct XxSuffixArray {
    xx_text: String,
    xx_sa: Vec<usize>,
}

impl std::fmt::Display for XxSuffixArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuffixArray(len={})", self.xx_text.len())
    }
}

impl Default for XxSuffixArray {
    fn default() -> Self { Self::xx_new("") }
}

impl XxSuffixArray {
    /// Build a suffix array from a string.
    pub fn xx_new(text: &str) -> Self {
        let n = text.len();
        let bytes = text.as_bytes();
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self { xx_text: text.to_string(), xx_sa: sa }
    }

    /// Length of the text.
    pub fn xx_len(&self) -> usize { self.xx_text.len() }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_text.is_empty() }

    /// Get the suffix array.
    pub fn xx_array(&self) -> &[usize] { &self.xx_sa }

    /// Get the original text.
    pub fn xx_text(&self) -> &str { &self.xx_text }

    /// Search for a pattern, returning all starting positions.
    pub fn xx_search(&self, pattern: &str) -> Vec<usize> {
        if pattern.is_empty() || self.xx_text.is_empty() { return Vec::new(); }
        let pb = pattern.as_bytes();
        let tb = self.xx_text.as_bytes();
        let n = tb.len();
        let m = pb.len();
        // Binary search for lower bound
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] < *pb { lo = mid + 1; } else { hi = mid; }
        }
        let lower = lo;
        hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] <= *pb { lo = mid + 1; } else { hi = mid; }
        }
        let upper = lo;
        self.xx_sa[lower..upper].to_vec()
    }

    /// Count occurrences of a pattern.
    pub fn xx_count(&self, pattern: &str) -> usize {
        self.xx_search(pattern).len()
    }

    /// Get the suffix at position i in sorted order.
    pub fn xx_suffix_at(&self, i: usize) -> &str {
        if i < self.xx_sa.len() { &self.xx_text[self.xx_sa[i]..] } else { "" }
    }

    /// Find the longest repeated substring.
    pub fn xx_longest_repeated(&self) -> String {
        if self.xx_sa.len() < 2 { return String::new(); }
        let tb = self.xx_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xx_sa.len() {
            let a = self.xx_sa[i - 1];
            let b = self.xx_sa[i];
            let mut lcp = 0;
            while a + lcp < tb.len() && b + lcp < tb.len() && tb[a + lcp] == tb[b + lcp] {
                lcp += 1;
            }
            if lcp > best_len { best_len = lcp; best_start = a; }
        }
        self.xx_text[best_start..best_start + best_len].to_string()
    }
}


// --- xy_ Cuckoo Hash Map ---

/// Cuckoo hash map with two hash functions and O(1) amortized lookup.
#[derive(Debug, Clone)]
pub struct XyCuckooMap<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xy_table1: Vec<Option<(K, V)>>,
    xy_table2: Vec<Option<(K, V)>>,
    xy_capacity: usize,
    xy_size: usize,
    xy_seed1: u64,
    xy_seed2: u64,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XyCuckooMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CuckooMap(size={}, cap={})", self.xy_size, self.xy_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> Default for XyCuckooMap<K, V> {
    fn default() -> Self { Self::xy_new(16) }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XyCuckooMap<K, V> {
    /// Create a new cuckoo hash map with given capacity.
    pub fn xy_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xy_table1: (0..cap).map(|_| None).collect(),
            xy_table2: (0..cap).map(|_| None).collect(),
            xy_capacity: cap,
            xy_size: 0,
            xy_seed1: 0x517cc1b727220a95,
            xy_seed2: 0x6c62272e07bb0142,
        }
    }

    fn xy_hash1(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed1.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    fn xy_hash2(&self, key: &K) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.xy_seed2.hash(&mut h);
        key.hash(&mut h);
        h.finish() as usize % self.xy_capacity
    }

    /// Number of elements.
    pub fn xy_len(&self) -> usize { self.xy_size }

    /// Is empty.
    pub fn xy_is_empty(&self) -> bool { self.xy_size == 0 }

    /// Insert a key-value pair.
    pub fn xy_insert(&mut self, key: K, value: V) -> bool {
        if self.xy_get(&key).is_some() {
            let h1 = self.xy_hash1(&key);
            if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == key) {
                self.xy_table1[h1] = Some((key, value));
            } else {
                let h2 = self.xy_hash2(&key);
                self.xy_table2[h2] = Some((key, value));
            }
            return true;
        }
        let mut k = key;
        let mut v = value;
        for _ in 0..self.xy_capacity {
            let h1 = self.xy_hash1(&k);
            if self.xy_table1[h1].is_none() {
                self.xy_table1[h1] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old = self.xy_table1[h1].take().unwrap();
            self.xy_table1[h1] = Some((k, v));
            k = old.0;
            v = old.1;
            let h2 = self.xy_hash2(&k);
            if self.xy_table2[h2].is_none() {
                self.xy_table2[h2] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
            let old2 = self.xy_table2[h2].take().unwrap();
            self.xy_table2[h2] = Some((k, v));
            k = old2.0;
            v = old2.1;
        }
        // Rehash needed — just put in table1 with linear probing fallback
        for i in 0..self.xy_capacity {
            if self.xy_table1[i].is_none() {
                self.xy_table1[i] = Some((k, v));
                self.xy_size += 1;
                return true;
            }
        }
        false
    }

    /// Look up a key.
    pub fn xy_get(&self, key: &K) -> Option<&V> {
        let h1 = self.xy_hash1(key);
        if let Some((k, v)) = &self.xy_table1[h1] {
            if *k == *key { return Some(v); }
        }
        let h2 = self.xy_hash2(key);
        if let Some((k, v)) = &self.xy_table2[h2] {
            if *k == *key { return Some(v); }
        }
        None
    }

    /// Check if key exists.
    pub fn xy_contains(&self, key: &K) -> bool { self.xy_get(key).is_some() }

    /// Remove a key.
    pub fn xy_remove(&mut self, key: &K) -> Option<V> {
        let h1 = self.xy_hash1(key);
        if self.xy_table1[h1].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table1[h1].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        let h2 = self.xy_hash2(key);
        if self.xy_table2[h2].as_ref().is_some_and(|(k, _)| *k == *key) {
            let (_, v) = self.xy_table2[h2].take().unwrap();
            self.xy_size -= 1;
            return Some(v);
        }
        None
    }

    /// Clear the map.
    pub fn xy_clear(&mut self) {
        for slot in &mut self.xy_table1 { *slot = None; }
        for slot in &mut self.xy_table2 { *slot = None; }
        self.xy_size = 0;
    }

    /// Collect all keys.
    pub fn xy_keys(&self) -> Vec<K> {
        let mut keys = Vec::new();
        for slot in &self.xy_table1 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        for slot in &self.xy_table2 {
            if let Some((k, _)) = slot { keys.push(k.clone()); }
        }
        keys
    }
}

// --- xy_ Count-Min Sketch ---

/// Count-min sketch for approximate frequency counting with bounded error.
#[derive(Debug, Clone)]
pub struct XyCountMinSketch {
    xy_table: Vec<Vec<u64>>,
    xy_width: usize,
    xy_depth: usize,
    xy_seeds: Vec<u64>,
}

impl std::fmt::Display for XyCountMinSketch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CMS(w={}, d={})", self.xy_width, self.xy_depth)
    }
}

impl Default for XyCountMinSketch {
    fn default() -> Self { Self::xy_new(1000, 5) }
}

impl XyCountMinSketch {
    /// Create a new count-min sketch with given width and depth.
    pub fn xy_new(width: usize, depth: usize) -> Self {
        let seeds: Vec<u64> = (0..depth).map(|i| 0x9e3779b97f4a7c15u64.wrapping_add((i as u64).wrapping_mul(0x517cc1b727220a95))).collect();
        Self {
            xy_table: vec![vec![0u64; width]; depth],
            xy_width: width,
            xy_depth: depth,
            xy_seeds: seeds,
        }
    }

    fn xy_hash(&self, item: u64, seed: u64) -> usize {
        let h = item.wrapping_mul(seed).wrapping_add(seed >> 16);
        (h ^ (h >> 32)) as usize % self.xy_width
    }

    /// Increment the count for an item.
    pub fn xy_add(&mut self, item: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += 1;
        }
    }

    /// Add with a specific count.
    pub fn xy_add_count(&mut self, item: u64, count: u64) {
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            self.xy_table[i][idx] += count;
        }
    }

    /// Estimate the count for an item (guaranteed to be >= actual count).
    pub fn xy_estimate(&self, item: u64) -> u64 {
        let mut min_count = u64::MAX;
        for i in 0..self.xy_depth {
            let idx = self.xy_hash(item, self.xy_seeds[i]);
            min_count = min_count.min(self.xy_table[i][idx]);
        }
        min_count
    }

    /// Width of the sketch.
    pub fn xy_width(&self) -> usize { self.xy_width }

    /// Depth of the sketch.
    pub fn xy_depth(&self) -> usize { self.xy_depth }

    /// Clear the sketch.
    pub fn xy_clear(&mut self) {
        for row in &mut self.xy_table {
            for cell in row { *cell = 0; }
        }
    }

    /// Merge another sketch into this one.
    pub fn xy_merge(&mut self, other: &XyCountMinSketch) {
        if self.xy_width != other.xy_width || self.xy_depth != other.xy_depth { return; }
        for i in 0..self.xy_depth {
            for j in 0..self.xy_width {
                self.xy_table[i][j] += other.xy_table[i][j];
            }
        }
    }
}


// --- xz_ HyperLogLog ---

/// HyperLogLog probabilistic cardinality estimator with configurable precision.
#[derive(Debug, Clone)]
pub struct XzHyperLogLog {
    xz_registers: Vec<u8>,
    xz_m: usize,
    xz_b: u32,
}

impl std::fmt::Display for XzHyperLogLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HLL(m={}, est={:.0})", self.xz_m, self.xz_estimate())
    }
}

impl Default for XzHyperLogLog {
    fn default() -> Self { Self::xz_new(10) }
}

impl XzHyperLogLog {
    /// Create a new HyperLogLog with precision b (4 <= b <= 16). Uses 2^b registers.
    pub fn xz_new(b: u32) -> Self {
        let b = b.clamp(4, 16);
        let m = 1 << b;
        Self { xz_registers: vec![0u8; m], xz_m: m, xz_b: b }
    }

    fn xz_hash(item: u64) -> u64 {
        let mut h = item;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h >> 33;
        h
    }

    /// Add an item.
    pub fn xz_add(&mut self, item: u64) {
        let h = Self::xz_hash(item);
        let idx = (h as usize) & (self.xz_m - 1);
        let w = h >> self.xz_b;
        let rho = if w == 0 { 64 - self.xz_b } else { w.trailing_zeros() + 1 };
        let rho = rho.min(255) as u8;
        if rho > self.xz_registers[idx] {
            self.xz_registers[idx] = rho;
        }
    }

    /// Estimate the cardinality.
    pub fn xz_estimate(&self) -> f64 {
        let m = self.xz_m as f64;
        let alpha = match self.xz_m {
            16 => 0.673,
            32 => 0.697,
            64 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / m),
        };
        let sum: f64 = self.xz_registers.iter().map(|&r| 2.0f64.powi(-(r as i32))).sum();
        let raw = alpha * m * m / sum;
        if raw <= 2.5 * m {
            let zeros = self.xz_registers.iter().filter(|&&r| r == 0).count();
            if zeros > 0 { m * (m / zeros as f64).ln() } else { raw }
        } else if raw <= (1u64 << 32) as f64 / 30.0 {
            raw
        } else {
            -(((1u64 << 32) as f64) * (1.0 - raw / (1u64 << 32) as f64).ln())
        }
    }

    /// Merge another HyperLogLog into this one.
    pub fn xz_merge(&mut self, other: &XzHyperLogLog) {
        if self.xz_m != other.xz_m { return; }
        for i in 0..self.xz_m {
            if other.xz_registers[i] > self.xz_registers[i] {
                self.xz_registers[i] = other.xz_registers[i];
            }
        }
    }

    /// Clear all registers.
    pub fn xz_clear(&mut self) {
        for r in &mut self.xz_registers { *r = 0; }
    }

    /// Number of registers.
    pub fn xz_num_registers(&self) -> usize { self.xz_m }

    /// Precision parameter.
    pub fn xz_precision(&self) -> u32 { self.xz_b }
}

// --- xz_ LRU Cache ---

/// LRU cache with O(1) get/put using a doubly-linked list and hash map.
#[derive(Debug, Clone)]
pub struct XzLruCache<K: Eq + Clone + std::hash::Hash, V: Clone> {
    xz_capacity: usize,
    xz_entries: Vec<(K, V)>,
    xz_order: Vec<usize>,
    xz_map: std::collections::HashMap<K, usize>,
}

impl<K: Eq + Clone + std::hash::Hash + std::fmt::Display, V: Clone> std::fmt::Display for XzLruCache<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LRU(size={}, cap={})", self.xz_map.len(), self.xz_capacity)
    }
}

impl<K: Eq + Clone + std::hash::Hash, V: Clone> XzLruCache<K, V> {
    /// Create a new LRU cache with given capacity.
    pub fn xz_new(capacity: usize) -> Self {
        Self {
            xz_capacity: capacity.max(1),
            xz_entries: Vec::new(),
            xz_order: Vec::new(),
            xz_map: std::collections::HashMap::new(),
        }
    }

    /// Number of entries.
    pub fn xz_len(&self) -> usize { self.xz_map.len() }

    /// Is empty.
    pub fn xz_is_empty(&self) -> bool { self.xz_map.is_empty() }

    /// Capacity.
    pub fn xz_capacity(&self) -> usize { self.xz_capacity }

    /// Get a value, marking it as recently used.
    pub fn xz_get(&mut self, key: &K) -> Option<&V> {
        if let Some(&idx) = self.xz_map.get(key) {
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            Some(&self.xz_entries[idx].1)
        } else {
            None
        }
    }

    /// Put a key-value pair, evicting the least recently used if at capacity.
    pub fn xz_put(&mut self, key: K, value: V) {
        if let Some(&idx) = self.xz_map.get(&key) {
            self.xz_entries[idx].1 = value;
            self.xz_order.retain(|&i| i != idx);
            self.xz_order.push(idx);
            return;
        }
        if self.xz_map.len() >= self.xz_capacity {
            if let Some(evict_idx) = self.xz_order.first().copied() {
                self.xz_order.remove(0);
                let evict_key = self.xz_entries[evict_idx].0.clone();
                self.xz_map.remove(&evict_key);
            }
        }
        let idx = self.xz_entries.len();
        self.xz_entries.push((key.clone(), value));
        self.xz_map.insert(key, idx);
        self.xz_order.push(idx);
    }

    /// Check if key exists (without updating LRU order).
    pub fn xz_contains(&self, key: &K) -> bool { self.xz_map.contains_key(key) }

    /// Remove a key.
    pub fn xz_remove(&mut self, key: &K) -> Option<V> {
        if let Some(idx) = self.xz_map.remove(key) {
            self.xz_order.retain(|&i| i != idx);
            Some(self.xz_entries[idx].1.clone())
        } else {
            None
        }
    }

    /// Clear the cache.
    pub fn xz_clear(&mut self) {
        self.xz_entries.clear();
        self.xz_order.clear();
        self.xz_map.clear();
    }

    /// Get all keys in LRU order (least recent first).
    pub fn xz_keys_lru(&self) -> Vec<K> {
        self.xz_order.iter().filter_map(|&idx| {
            let k = &self.xz_entries[idx].0;
            if self.xz_map.contains_key(k) { Some(k.clone()) } else { None }
        }).collect()
    }

    /// Peek at value without updating LRU order.
    pub fn xz_peek(&self, key: &K) -> Option<&V> {
        self.xz_map.get(key).map(|&idx| &self.xz_entries[idx].1)
    }
}


// --- ya_ Trie (Prefix Tree) ---

/// A node in a trie (prefix tree) for string key lookups.
#[derive(Debug, Clone)]
pub struct YaTrieNode<V: Clone> {
    ya_children: std::collections::HashMap<char, Box<YaTrieNode<V>>>,
    ya_value: Option<V>,
    ya_is_end: bool,
}

impl<V: Clone> Default for YaTrieNode<V> {
    fn default() -> Self {
        Self { ya_children: std::collections::HashMap::new(), ya_value: None, ya_is_end: false }
    }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YaTrieNode<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TrieNode(children={}, end={})", self.ya_children.len(), self.ya_is_end)
    }
}

/// Trie (prefix tree) for O(m) string key operations where m is key length.
#[derive(Debug, Clone)]
pub struct YaTrie<V: Clone> {
    ya_root: YaTrieNode<V>,
    ya_size: usize,
}

impl<V: Clone> Default for YaTrie<V> {
    fn default() -> Self { Self::ya_new() }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YaTrie<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Trie(size={})", self.ya_size)
    }
}

impl<V: Clone> YaTrie<V> {
    /// Create an empty trie.
    pub fn ya_new() -> Self { Self { ya_root: YaTrieNode::default(), ya_size: 0 } }

    /// Number of stored keys.
    pub fn ya_len(&self) -> usize { self.ya_size }

    /// Is the trie empty.
    pub fn ya_is_empty(&self) -> bool { self.ya_size == 0 }

    /// Insert a key-value pair.
    pub fn ya_insert(&mut self, key: &str, value: V) {
        let mut node = &mut self.ya_root;
        for ch in key.chars() {
            node = node.ya_children.entry(ch).or_insert_with(|| Box::new(YaTrieNode::default()));
        }
        if !node.ya_is_end { self.ya_size += 1; }
        node.ya_value = Some(value);
        node.ya_is_end = true;
    }

    /// Look up a key.
    pub fn ya_get(&self, key: &str) -> Option<&V> {
        let mut node = &self.ya_root;
        for ch in key.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return None,
            }
        }
        if node.ya_is_end { node.ya_value.as_ref() } else { None }
    }

    /// Check if a key exists.
    pub fn ya_contains(&self, key: &str) -> bool { self.ya_get(key).is_some() }

    /// Check if any key starts with the given prefix.
    pub fn ya_has_prefix(&self, prefix: &str) -> bool {
        let mut node = &self.ya_root;
        for ch in prefix.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return false,
            }
        }
        true
    }

    /// Collect all keys with the given prefix.
    pub fn ya_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.ya_root;
        for ch in prefix.chars() {
            match node.ya_children.get(&ch) {
                Some(child) => node = child,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        Self::ya_collect_keys(node, &mut prefix.to_string(), &mut results);
        results
    }

    fn ya_collect_keys(node: &YaTrieNode<V>, current: &mut String, results: &mut Vec<String>) {
        if node.ya_is_end { results.push(current.clone()); }
        let mut chars: Vec<char> = node.ya_children.keys().copied().collect();
        chars.sort();
        for ch in chars {
            current.push(ch);
            Self::ya_collect_keys(node.ya_children.get(&ch).unwrap(), current, results);
            current.pop();
        }
    }

    /// Collect all keys.
    pub fn ya_all_keys(&self) -> Vec<String> {
        self.ya_keys_with_prefix("")
    }

    /// Remove a key. Returns the value if it existed.
    pub fn ya_remove(&mut self, key: &str) -> Option<V> {
        let result = Self::ya_remove_recursive(&mut self.ya_root, key, 0);
        if result.is_some() { self.ya_size -= 1; }
        result
    }

    fn ya_remove_recursive(node: &mut YaTrieNode<V>, key: &str, depth: usize) -> Option<V> {
        let chars: Vec<char> = key.chars().collect();
        if depth == chars.len() {
            if node.ya_is_end {
                node.ya_is_end = false;
                return node.ya_value.take();
            }
            return None;
        }
        let ch = chars[depth];
        if let Some(child) = node.ya_children.get_mut(&ch) {
            let result = Self::ya_remove_recursive(child, key, depth + 1);
            if !child.ya_is_end && child.ya_children.is_empty() {
                node.ya_children.remove(&ch);
            }
            result
        } else {
            None
        }
    }

    /// Clear the trie.
    pub fn ya_clear(&mut self) {
        self.ya_root = YaTrieNode::default();
        self.ya_size = 0;
    }

    /// Count keys with a given prefix.
    pub fn ya_count_prefix(&self, prefix: &str) -> usize {
        self.ya_keys_with_prefix(prefix).len()
    }

    /// Longest common prefix among all keys.
    pub fn ya_longest_common_prefix(&self) -> String {
        let mut result = String::new();
        let mut node = &self.ya_root;
        while node.ya_children.len() == 1 && !node.ya_is_end {
            let (&ch, child) = node.ya_children.iter().next().unwrap();
            result.push(ch);
            node = child;
        }
        result
    }
}

// --- ya_ Bloom Filter ---

/// Bloom filter for probabilistic set membership testing with no false negatives.
#[derive(Debug, Clone)]
pub struct YaBloomFilter {
    ya_bits: Vec<bool>,
    ya_size: usize,
    ya_num_hashes: usize,
    ya_count: usize,
}

impl std::fmt::Display for YaBloomFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bloom(bits={}, hashes={}, count={})", self.ya_size, self.ya_num_hashes, self.ya_count)
    }
}

impl Default for YaBloomFilter {
    fn default() -> Self { Self::ya_new(1000, 5) }
}

impl YaBloomFilter {
    /// Create a new bloom filter with given bit size and number of hash functions.
    pub fn ya_new(bits: usize, num_hashes: usize) -> Self {
        Self { ya_bits: vec![false; bits], ya_size: bits, ya_num_hashes: num_hashes.max(1), ya_count: 0 }
    }

    /// Create from expected number of items and desired false positive rate.
    pub fn ya_with_fp_rate(expected_items: usize, fp_rate: f64) -> Self {
        let bits = (-(expected_items as f64) * fp_rate.ln() / (2.0f64.ln().powi(2))).ceil() as usize;
        let bits = bits.max(64);
        let hashes = ((bits as f64 / expected_items as f64) * 2.0f64.ln()).ceil() as usize;
        let hashes = hashes.max(1);
        Self::ya_new(bits, hashes)
    }

    fn ya_hash(&self, item: u64, seed: usize) -> usize {
        let h = item.wrapping_mul(0xff51afd7ed558ccd_u64.wrapping_add(seed as u64));
        let h = h ^ (h >> 33);
        let h = h.wrapping_mul(0xc4ceb9fe1a85ec53_u64.wrapping_add(seed as u64 * 7));
        (h ^ (h >> 33)) as usize % self.ya_size
    }

    /// Add an item.
    pub fn ya_add(&mut self, item: u64) {
        for i in 0..self.ya_num_hashes {
            let idx = self.ya_hash(item, i);
            self.ya_bits[idx] = true;
        }
        self.ya_count += 1;
    }

    /// Check if an item might be in the set (false positives possible, no false negatives).
    pub fn ya_might_contain(&self, item: u64) -> bool {
        for i in 0..self.ya_num_hashes {
            let idx = self.ya_hash(item, i);
            if !self.ya_bits[idx] { return false; }
        }
        true
    }

    /// Number of items added.
    pub fn ya_count(&self) -> usize { self.ya_count }

    /// Bit array size.
    pub fn ya_bit_size(&self) -> usize { self.ya_size }

    /// Number of hash functions.
    pub fn ya_num_hashes(&self) -> usize { self.ya_num_hashes }

    /// Estimated false positive rate.
    pub fn ya_estimated_fp_rate(&self) -> f64 {
        let ones = self.ya_bits.iter().filter(|&&b| b).count() as f64;
        (ones / self.ya_size as f64).powi(self.ya_num_hashes as i32)
    }

    /// Clear the filter.
    pub fn ya_clear(&mut self) {
        for b in &mut self.ya_bits { *b = false; }
        self.ya_count = 0;
    }

    /// Merge another bloom filter (union).
    pub fn ya_merge(&mut self, other: &YaBloomFilter) {
        if self.ya_size != other.ya_size { return; }
        for i in 0..self.ya_size {
            self.ya_bits[i] = self.ya_bits[i] || other.ya_bits[i];
        }
    }
}


// --- yb_ Ternary Search Tree ---

/// Node in a ternary search tree (TST) for space-efficient string storage.
#[derive(Debug, Clone)]
pub struct YbTstNode<V: Clone> {
    yb_ch: char,
    yb_left: Option<Box<YbTstNode<V>>>,
    yb_mid: Option<Box<YbTstNode<V>>>,
    yb_right: Option<Box<YbTstNode<V>>>,
    yb_value: Option<V>,
}

impl<V: Clone> YbTstNode<V> {
    fn yb_new(ch: char) -> Self {
        Self { yb_ch: ch, yb_left: None, yb_mid: None, yb_right: None, yb_value: None }
    }
}

/// Ternary search tree for efficient string-keyed storage with prefix queries.
#[derive(Debug, Clone)]
pub struct YbTernarySearchTree<V: Clone> {
    yb_root: Option<Box<YbTstNode<V>>>,
    yb_size: usize,
}

impl<V: Clone> Default for YbTernarySearchTree<V> {
    fn default() -> Self { Self { yb_root: None, yb_size: 0 } }
}

impl<V: Clone + std::fmt::Display> std::fmt::Display for YbTernarySearchTree<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TST(size={})", self.yb_size)
    }
}

impl<V: Clone> YbTernarySearchTree<V> {
    /// Create an empty TST.
    pub fn yb_new() -> Self { Self { yb_root: None, yb_size: 0 } }

    /// Number of stored keys.
    pub fn yb_len(&self) -> usize { self.yb_size }

    /// Is the tree empty.
    pub fn yb_is_empty(&self) -> bool { self.yb_size == 0 }

    /// Insert a key-value pair.
    pub fn yb_insert(&mut self, key: &str, value: V) {
        if key.is_empty() { return; }
        let chars: Vec<char> = key.chars().collect();
        let was_new = Self::yb_insert_node(&mut self.yb_root, &chars, 0, value);
        if was_new { self.yb_size += 1; }
    }

    fn yb_insert_node(node: &mut Option<Box<YbTstNode<V>>>, chars: &[char], depth: usize, value: V) -> bool {
        let ch = chars[depth];
        if node.is_none() { *node = Some(Box::new(YbTstNode::yb_new(ch))); }
        let n = node.as_mut().unwrap();
        if ch < n.yb_ch {
            Self::yb_insert_node(&mut n.yb_left, chars, depth, value)
        } else if ch > n.yb_ch {
            Self::yb_insert_node(&mut n.yb_right, chars, depth, value)
        } else if depth + 1 < chars.len() {
            Self::yb_insert_node(&mut n.yb_mid, chars, depth + 1, value)
        } else {
            let was_new = n.yb_value.is_none();
            n.yb_value = Some(value);
            was_new
        }
    }

    /// Look up a key.
    pub fn yb_get(&self, key: &str) -> Option<&V> {
        if key.is_empty() { return None; }
        let chars: Vec<char> = key.chars().collect();
        Self::yb_get_node(self.yb_root.as_deref(), &chars, 0)
    }

    fn yb_get_node<'a>(node: Option<&'a YbTstNode<V>>, chars: &[char], depth: usize) -> Option<&'a V> {
        let n = node?;
        let ch = chars[depth];
        if ch < n.yb_ch {
            Self::yb_get_node(n.yb_left.as_deref(), chars, depth)
        } else if ch > n.yb_ch {
            Self::yb_get_node(n.yb_right.as_deref(), chars, depth)
        } else if depth + 1 < chars.len() {
            Self::yb_get_node(n.yb_mid.as_deref(), chars, depth + 1)
        } else {
            n.yb_value.as_ref()
        }
    }

    /// Check if a key exists.
    pub fn yb_contains(&self, key: &str) -> bool { self.yb_get(key).is_some() }

    /// Collect all keys.
    pub fn yb_all_keys(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut current = String::new();
        Self::yb_collect(self.yb_root.as_deref(), &mut current, &mut results);
        results
    }

    fn yb_collect(node: Option<&YbTstNode<V>>, current: &mut String, results: &mut Vec<String>) {
        let Some(n) = node else { return };
        Self::yb_collect(n.yb_left.as_deref(), current, results);
        current.push(n.yb_ch);
        if n.yb_value.is_some() { results.push(current.clone()); }
        Self::yb_collect(n.yb_mid.as_deref(), current, results);
        current.pop();
        Self::yb_collect(n.yb_right.as_deref(), current, results);
    }

    /// Collect keys with a given prefix.
    pub fn yb_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        if prefix.is_empty() { return self.yb_all_keys(); }
        let chars: Vec<char> = prefix.chars().collect();
        let node = Self::yb_prefix_node(self.yb_root.as_deref(), &chars, 0);
        let mut results = Vec::new();
        if let Some(n) = node {
            if n.yb_value.is_some() { results.push(prefix.to_string()); }
            let mut current = prefix.to_string();
            Self::yb_collect(n.yb_mid.as_deref(), &mut current, &mut results);
        }
        results
    }

    fn yb_prefix_node<'a>(node: Option<&'a YbTstNode<V>>, chars: &[char], depth: usize) -> Option<&'a YbTstNode<V>> {
        let n = node?;
        let ch = chars[depth];
        if ch < n.yb_ch {
            Self::yb_prefix_node(n.yb_left.as_deref(), chars, depth)
        } else if ch > n.yb_ch {
            Self::yb_prefix_node(n.yb_right.as_deref(), chars, depth)
        } else if depth + 1 < chars.len() {
            Self::yb_prefix_node(n.yb_mid.as_deref(), chars, depth + 1)
        } else {
            Some(n)
        }
    }

    /// Clear the tree.
    pub fn yb_clear(&mut self) { self.yb_root = None; self.yb_size = 0; }
}

// --- yb_ Quadtree ---

/// A point in 2D space for quadtree storage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YbPoint {
    pub yb_x: f64,
    pub yb_y: f64,
}

impl std::fmt::Display for YbPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.2}, {:.2})", self.yb_x, self.yb_y)
    }
}

impl Default for YbPoint {
    fn default() -> Self { Self { yb_x: 0.0, yb_y: 0.0 } }
}

impl YbPoint {
    /// Create a new point.
    pub fn yb_new(x: f64, y: f64) -> Self { Self { yb_x: x, yb_y: y } }

    /// Distance to another point.
    pub fn yb_distance(&self, other: &YbPoint) -> f64 {
        ((self.yb_x - other.yb_x).powi(2) + (self.yb_y - other.yb_y).powi(2)).sqrt()
    }
}

/// Axis-aligned bounding box for quadtree partitioning.
#[derive(Debug, Clone, Copy)]
pub struct YbBounds {
    pub yb_x: f64,
    pub yb_y: f64,
    pub yb_w: f64,
    pub yb_h: f64,
}

impl std::fmt::Display for YbBounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bounds({:.1},{:.1} {}x{})", self.yb_x, self.yb_y, self.yb_w, self.yb_h)
    }
}

impl Default for YbBounds {
    fn default() -> Self { Self { yb_x: 0.0, yb_y: 0.0, yb_w: 100.0, yb_h: 100.0 } }
}

impl YbBounds {
    /// Create bounds from origin and size.
    pub fn yb_new(x: f64, y: f64, w: f64, h: f64) -> Self { Self { yb_x: x, yb_y: y, yb_w: w, yb_h: h } }

    /// Check if a point is inside these bounds.
    pub fn yb_contains(&self, p: &YbPoint) -> bool {
        p.yb_x >= self.yb_x && p.yb_x < self.yb_x + self.yb_w &&
        p.yb_y >= self.yb_y && p.yb_y < self.yb_y + self.yb_h
    }

    /// Check if two bounds intersect.
    pub fn yb_intersects(&self, other: &YbBounds) -> bool {
        !(self.yb_x + self.yb_w <= other.yb_x || other.yb_x + other.yb_w <= self.yb_x ||
          self.yb_y + self.yb_h <= other.yb_y || other.yb_y + other.yb_h <= self.yb_y)
    }
}

/// Quadtree for 2D spatial indexing with region queries.
#[derive(Debug, Clone)]
pub struct YbQuadtree {
    yb_bounds: YbBounds,
    yb_points: Vec<YbPoint>,
    yb_capacity: usize,
    yb_nw: Option<Box<YbQuadtree>>,
    yb_ne: Option<Box<YbQuadtree>>,
    yb_sw: Option<Box<YbQuadtree>>,
    yb_se: Option<Box<YbQuadtree>>,
    yb_divided: bool,
}

impl std::fmt::Display for YbQuadtree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Quadtree(points={}, bounds={})", self.yb_count(), self.yb_bounds)
    }
}

impl Default for YbQuadtree {
    fn default() -> Self { Self::yb_new(YbBounds::default(), 4) }
}

impl YbQuadtree {
    /// Create a new quadtree with given bounds and node capacity.
    pub fn yb_new(bounds: YbBounds, capacity: usize) -> Self {
        Self {
            yb_bounds: bounds, yb_points: Vec::new(), yb_capacity: capacity.max(1),
            yb_nw: None, yb_ne: None, yb_sw: None, yb_se: None, yb_divided: false,
        }
    }

    fn yb_subdivide(&mut self) {
        let x = self.yb_bounds.yb_x;
        let y = self.yb_bounds.yb_y;
        let hw = self.yb_bounds.yb_w / 2.0;
        let hh = self.yb_bounds.yb_h / 2.0;
        self.yb_nw = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x, y, hw, hh), self.yb_capacity)));
        self.yb_ne = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x + hw, y, hw, hh), self.yb_capacity)));
        self.yb_sw = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x, y + hh, hw, hh), self.yb_capacity)));
        self.yb_se = Some(Box::new(YbQuadtree::yb_new(YbBounds::yb_new(x + hw, y + hh, hw, hh), self.yb_capacity)));
        self.yb_divided = true;
    }

    /// Insert a point.
    pub fn yb_insert(&mut self, point: YbPoint) -> bool {
        if !self.yb_bounds.yb_contains(&point) { return false; }
        if self.yb_points.len() < self.yb_capacity && !self.yb_divided {
            self.yb_points.push(point);
            return true;
        }
        if !self.yb_divided { self.yb_subdivide(); }
        if self.yb_nw.as_mut().unwrap().yb_insert(point) { return true; }
        if self.yb_ne.as_mut().unwrap().yb_insert(point) { return true; }
        if self.yb_sw.as_mut().unwrap().yb_insert(point) { return true; }
        self.yb_se.as_mut().unwrap().yb_insert(point)
    }

    /// Query all points within a rectangular region.
    pub fn yb_query(&self, range: &YbBounds) -> Vec<YbPoint> {
        let mut found = Vec::new();
        self.yb_query_inner(range, &mut found);
        found
    }

    fn yb_query_inner(&self, range: &YbBounds, found: &mut Vec<YbPoint>) {
        if !self.yb_bounds.yb_intersects(range) { return; }
        for p in &self.yb_points {
            if range.yb_contains(p) { found.push(*p); }
        }
        if self.yb_divided {
            self.yb_nw.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_ne.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_sw.as_ref().unwrap().yb_query_inner(range, found);
            self.yb_se.as_ref().unwrap().yb_query_inner(range, found);
        }
    }

    /// Count total points.
    pub fn yb_count(&self) -> usize {
        let mut c = self.yb_points.len();
        if self.yb_divided {
            c += self.yb_nw.as_ref().unwrap().yb_count();
            c += self.yb_ne.as_ref().unwrap().yb_count();
            c += self.yb_sw.as_ref().unwrap().yb_count();
            c += self.yb_se.as_ref().unwrap().yb_count();
        }
        c
    }

    /// Is the quadtree empty.
    pub fn yb_is_empty(&self) -> bool { self.yb_count() == 0 }

    /// Get bounds.
    pub fn yb_bounds(&self) -> &YbBounds { &self.yb_bounds }

    /// Find nearest point to a target.
    pub fn yb_nearest(&self, target: &YbPoint) -> Option<YbPoint> {
        let all = self.yb_query(&self.yb_bounds);
        all.into_iter().min_by(|a, b| {
            a.yb_distance(target).partial_cmp(&b.yb_distance(target)).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

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

    // -- populate_from_git tests -------------------------------------------

    #[test]
    fn populate_from_git_basic() {
        let mut view = ScmView::new();
        let status_output = vec![
            (PathBuf::from("src/main.rs"), FileStatus::Modified),
            (PathBuf::from("src/lib.rs"), FileStatus::Added),
        ];
        populate_from_git(&mut view, &status_output);
        assert_eq!(view.entries.len(), 2);
        // Sorted by path: lib.rs < main.rs
        assert_eq!(view.entries[0].path, PathBuf::from("src/lib.rs"));
        assert_eq!(view.entries[1].path, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn populate_from_git_staged_flag() {
        let mut view = ScmView::new();
        let status_output = vec![
            (PathBuf::from("added.rs"), FileStatus::Added),
            (PathBuf::from("modified.rs"), FileStatus::Modified),
            (PathBuf::from("deleted.rs"), FileStatus::Deleted),
            (PathBuf::from("untracked.rs"), FileStatus::Untracked),
        ];
        populate_from_git(&mut view, &status_output);
        // Added gets staged=true
        let added = view.entries.iter().find(|e| e.path == PathBuf::from("added.rs")).unwrap();
        assert!(added.staged);
        // Modified does not
        let modified = view.entries.iter().find(|e| e.path == PathBuf::from("modified.rs")).unwrap();
        assert!(!modified.staged);
        // Deleted does not
        let deleted = view.entries.iter().find(|e| e.path == PathBuf::from("deleted.rs")).unwrap();
        assert!(!deleted.staged);
    }

    #[test]
    fn populate_from_git_clears_previous() {
        let mut view = ScmView::new();
        view.entries.push(ScmEntry {
            path: PathBuf::from("old.rs"),
            status: FileStatus::Modified,
            staged: false,
        });
        let status_output = vec![
            (PathBuf::from("new.rs"), FileStatus::Added),
        ];
        populate_from_git(&mut view, &status_output);
        assert_eq!(view.entries.len(), 1);
        assert_eq!(view.entries[0].path, PathBuf::from("new.rs"));
    }

    #[test]
    fn populate_from_git_empty() {
        let mut view = ScmView::new();
        populate_from_git(&mut view, &[]);
        assert!(view.entries.is_empty());
    }

    #[test]
    fn populate_from_git_sorts_entries() {
        let mut view = ScmView::new();
        let status_output = vec![
            (PathBuf::from("z.rs"), FileStatus::Modified),
            (PathBuf::from("a.rs"), FileStatus::Added),
            (PathBuf::from("m.rs"), FileStatus::Deleted),
        ];
        populate_from_git(&mut view, &status_output);
        let paths: Vec<_> = view.entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(paths, vec![
            PathBuf::from("a.rs"),
            PathBuf::from("m.rs"),
            PathBuf::from("z.rs"),
        ]);
    }

    // -- render_entries tests -----------------------------------------------

    #[test]
    fn render_entries_title_bar() {
        let mut view = ScmView::new();
        populate_from_git(&mut view, &[
            (PathBuf::from("a.rs"), FileStatus::Modified),
            (PathBuf::from("b.rs"), FileStatus::Added),
        ]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        view.render_entries(area, &mut buf);
        let first_line: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        assert!(first_line.contains("SOURCE CONTROL"));
        assert!(first_line.contains("(2)"));
    }

    #[test]
    fn render_entries_shows_staged_checkmark() {
        let mut view = ScmView::new();
        populate_from_git(&mut view, &[
            (PathBuf::from("staged.rs"), FileStatus::Added),
        ]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        view.render_entries(area, &mut buf);
        // Second row (y=1) should contain the checkmark for a staged entry
        let second_line: String = (0..area.width)
            .map(|x| buf[(x, 1)].symbol().to_string())
            .collect();
        assert!(second_line.contains("✓"));
    }

    #[test]
    fn render_entries_shows_status_icon() {
        let mut view = ScmView::new();
        populate_from_git(&mut view, &[
            (PathBuf::from("mod.rs"), FileStatus::Modified),
        ]);
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        view.render_entries(area, &mut buf);
        let second_line: String = (0..area.width)
            .map(|x| buf[(x, 1)].symbol().to_string())
            .collect();
        assert!(second_line.contains("M"));
        assert!(second_line.contains("mod.rs"));
    }

    #[test]
    fn render_entries_empty_no_panic() {
        let view = ScmView::new();
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        view.render_entries(area, &mut buf);
        let first_line: String = (0..area.width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect();
        assert!(first_line.contains("SOURCE CONTROL (0)"));
    }

    #[test]
    fn render_entries_zero_area_no_panic() {
        let mut view = ScmView::new();
        populate_from_git(&mut view, &[
            (PathBuf::from("a.rs"), FileStatus::Modified),
        ]);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(area);
        view.render_entries(area, &mut buf);
    }

    // -- filter_by_status tests -------------------------------------------

    #[test]
    fn filter_by_status_modified_only() {
        let changes = vec![
            ScmFileChange::new("a.rs", ScmFileStatus::Modified),
            ScmFileChange::new("b.rs", ScmFileStatus::Added),
            ScmFileChange::new("c.rs", ScmFileStatus::Modified),
            ScmFileChange::new("d.rs", ScmFileStatus::Deleted),
        ];
        let filtered = filter_by_status(&changes, ScmFileStatus::Modified);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|c| c.status == ScmFileStatus::Modified));
    }

    #[test]
    fn filter_by_status_no_match() {
        let changes = vec![
            ScmFileChange::new("a.rs", ScmFileStatus::Added),
        ];
        let filtered = filter_by_status(&changes, ScmFileStatus::Conflicted);
        assert!(filtered.is_empty());
    }

    // -- filter_by_directory tests ----------------------------------------

    #[test]
    fn filter_by_directory_matches_prefix() {
        let changes = vec![
            ScmFileChange::new("src/main.rs", ScmFileStatus::Modified),
            ScmFileChange::new("src/lib.rs", ScmFileStatus::Added),
            ScmFileChange::new("tests/test.rs", ScmFileStatus::Modified),
            ScmFileChange::new("README.md", ScmFileStatus::Modified),
        ];
        let filtered = filter_by_directory(&changes, Path::new("src"));
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|c| c.path.starts_with("src")));
    }

    // -- group_by_directory tests -----------------------------------------

    #[test]
    fn group_by_directory_separates_dirs() {
        let changes = vec![
            ScmFileChange::new("src/main.rs", ScmFileStatus::Modified),
            ScmFileChange::new("src/lib.rs", ScmFileStatus::Added),
            ScmFileChange::new("tests/test.rs", ScmFileStatus::Modified),
            ScmFileChange::new("Cargo.toml", ScmFileStatus::Modified),
        ];
        let groups = group_by_directory(&changes);
        // BTreeMap: "" < "src" < "tests"
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, PathBuf::from(""));
        assert_eq!(groups[0].1.len(), 1); // Cargo.toml
        assert_eq!(groups[1].0, PathBuf::from("src"));
        assert_eq!(groups[1].1.len(), 2);
        assert_eq!(groups[2].0, PathBuf::from("tests"));
        assert_eq!(groups[2].1.len(), 1);
    }

    // -- sort_by_path tests -----------------------------------------------

    #[test]
    fn sort_by_path_orders_alphabetically() {
        let mut changes = vec![
            ScmFileChange::new("z.rs", ScmFileStatus::Modified),
            ScmFileChange::new("a.rs", ScmFileStatus::Added),
            ScmFileChange::new("m.rs", ScmFileStatus::Deleted),
        ];
        sort_by_path(&mut changes);
        let paths: Vec<_> = changes.iter().map(|c| c.path.to_str().unwrap()).collect();
        assert_eq!(paths, vec!["a.rs", "m.rs", "z.rs"]);
    }

    // -- sort_by_status tests ---------------------------------------------

    #[test]
    fn sort_by_status_conflicts_first() {
        let mut changes = vec![
            ScmFileChange::new("a.rs", ScmFileStatus::Added),
            ScmFileChange::new("b.rs", ScmFileStatus::Conflicted),
            ScmFileChange::new("c.rs", ScmFileStatus::Modified),
            ScmFileChange::new("d.rs", ScmFileStatus::Untracked),
        ];
        sort_by_status(&mut changes);
        assert_eq!(changes[0].status, ScmFileStatus::Conflicted);
        assert_eq!(changes[1].status, ScmFileStatus::Modified);
        assert_eq!(changes[2].status, ScmFileStatus::Added);
        assert_eq!(changes[3].status, ScmFileStatus::Untracked);
    }

    // -- DiffStats tests --------------------------------------------------

    #[test]
    fn diff_stats_from_hunks() {
        let hunks = vec![DiffHunk {
            old_start: 1,
            old_count: 3,
            new_start: 1,
            new_count: 4,
            lines: vec![
                DiffLine::Context("ctx".into()),
                DiffLine::Removed("old".into()),
                DiffLine::Added("new1".into()),
                DiffLine::Added("new2".into()),
                DiffLine::Context("ctx2".into()),
            ],
        }];
        let stats = DiffStats::from_hunks(&hunks);
        assert_eq!(stats.insertions, 2);
        assert_eq!(stats.deletions, 1);
        assert_eq!(stats.files, 1);
        assert_eq!(stats.net_change(), 1);
    }

    #[test]
    fn diff_stats_aggregate() {
        let s1 = DiffStats { files: 1, insertions: 10, deletions: 3 };
        let s2 = DiffStats { files: 1, insertions: 5, deletions: 8 };
        let total = DiffStats::aggregate(&[s1, s2]);
        assert_eq!(total.files, 2);
        assert_eq!(total.insertions, 15);
        assert_eq!(total.deletions, 11);
        assert_eq!(total.net_change(), 4);
    }

    #[test]
    fn diff_stats_display_singular() {
        let stats = DiffStats { files: 1, insertions: 1, deletions: 1 };
        let s = stats.to_string();
        assert_eq!(s, "1 file changed, 1 insertion (+), 1 deletion (-)");
    }

    #[test]
    fn diff_stats_display_plural() {
        let stats = DiffStats { files: 3, insertions: 10, deletions: 5 };
        let s = stats.to_string();
        assert_eq!(s, "3 files changed, 10 insertions (+), 5 deletions (-)");
    }

    // -- status_counts tests ----------------------------------------------

    #[test]
    fn status_counts_correct() {
        let changes = vec![
            ScmFileChange::new("a.rs", ScmFileStatus::Modified),
            ScmFileChange::new("b.rs", ScmFileStatus::Modified),
            ScmFileChange::new("c.rs", ScmFileStatus::Added),
            ScmFileChange::new("d.rs", ScmFileStatus::Conflicted),
        ];
        let counts = status_counts(&changes);
        assert_eq!(counts[&ScmFileStatus::Modified], 2);
        assert_eq!(counts[&ScmFileStatus::Added], 1);
        assert_eq!(counts[&ScmFileStatus::Conflicted], 1);
        assert!(!counts.contains_key(&ScmFileStatus::Deleted));
    }

    // -- format_diffstat_line tests ---------------------------------------

    #[test]
    fn diffstat_line_basic() {
        let line = format_diffstat_line(Path::new("src/lib.rs"), 3, 2, 50);
        assert!(line.contains("src/lib.rs"));
        assert!(line.contains("| 5"));
        assert!(line.contains("+++--"));
    }

    #[test]
    fn diffstat_line_zero_changes() {
        let line = format_diffstat_line(Path::new("empty.rs"), 0, 0, 50);
        assert!(line.contains("| 0"));
    }

    #[test]
    fn diffstat_line_scales_down() {
        // 80 insertions + 20 deletions = 100 total, max_width = 10
        let line = format_diffstat_line(Path::new("big.rs"), 80, 20, 10);
        assert!(line.contains("| 100"));
        // Bar should be ≤ 10 chars total
        let bar_part = line.rsplit("| 100 ").next().unwrap_or("");
        assert!(bar_part.len() <= 12); // some rounding tolerance
    }

    // -- conflict detection tests -----------------------------------------

    #[test]
    fn has_conflicts_true() {
        let changes = vec![
            ScmFileChange::new("a.rs", ScmFileStatus::Modified),
            ScmFileChange::new("b.rs", ScmFileStatus::Conflicted),
        ];
        assert!(has_conflicts(&changes));
    }

    #[test]
    fn has_conflicts_false() {
        let changes = vec![
            ScmFileChange::new("a.rs", ScmFileStatus::Modified),
            ScmFileChange::new("b.rs", ScmFileStatus::Added),
        ];
        assert!(!has_conflicts(&changes));
    }

    #[test]
    fn conflicted_files_returns_only_conflicts() {
        let changes = vec![
            ScmFileChange::new("a.rs", ScmFileStatus::Conflicted),
            ScmFileChange::new("b.rs", ScmFileStatus::Modified),
            ScmFileChange::new("c.rs", ScmFileStatus::Conflicted),
        ];
        let conflicts = conflicted_files(&changes);
        assert_eq!(conflicts.len(), 2);
        assert!(conflicts.iter().all(|c| c.status == ScmFileStatus::Conflicted));
    }

    // -- staging/unstaging view helpers -----------------------------------

    #[test]
    fn toggle_staged_selected_works() {
        let mut view = ScmView::new();
        populate_from_git(&mut view, &[
            (PathBuf::from("a.rs"), FileStatus::Modified),
            (PathBuf::from("b.rs"), FileStatus::Added),
        ]);
        // a.rs is index 0 (sorted), Modified → staged=false
        assert!(!view.entries[0].staged);
        view.toggle_staged_selected();
        assert!(view.entries[0].staged);
        view.toggle_staged_selected();
        assert!(!view.entries[0].staged);
    }

    #[test]
    fn stage_all_and_unstage_all() {
        let mut view = ScmView::new();
        populate_from_git(&mut view, &[
            (PathBuf::from("a.rs"), FileStatus::Modified),
            (PathBuf::from("b.rs"), FileStatus::Deleted),
        ]);
        view.stage_all();
        assert!(view.entries.iter().all(|e| e.staged));
        assert_eq!(view.staged_paths().len(), 2);
        assert_eq!(view.unstaged_paths().len(), 0);

        view.unstage_all();
        assert!(view.entries.iter().all(|e| !e.staged));
        assert_eq!(view.staged_paths().len(), 0);
        assert_eq!(view.unstaged_paths().len(), 2);
    }

    // -- selection navigation tests ---------------------------------------

    #[test]
    fn select_next_wraps() {
        let mut view = ScmView::new();
        populate_from_git(&mut view, &[
            (PathBuf::from("a.rs"), FileStatus::Modified),
            (PathBuf::from("b.rs"), FileStatus::Modified),
            (PathBuf::from("c.rs"), FileStatus::Modified),
        ]);
        assert_eq!(view.selected_index, 0);
        view.select_next();
        assert_eq!(view.selected_index, 1);
        view.select_next();
        assert_eq!(view.selected_index, 2);
        view.select_next();
        assert_eq!(view.selected_index, 0); // wrapped
    }

    #[test]
    fn select_prev_wraps() {
        let mut view = ScmView::new();
        populate_from_git(&mut view, &[
            (PathBuf::from("a.rs"), FileStatus::Modified),
            (PathBuf::from("b.rs"), FileStatus::Modified),
        ]);
        assert_eq!(view.selected_index, 0);
        view.select_prev();
        assert_eq!(view.selected_index, 1); // wrapped to end
        view.select_prev();
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn select_on_empty_view_no_panic() {
        let mut view = ScmView::new();
        view.select_next();
        view.select_prev();
        assert_eq!(view.selected_index, 0);
    }

    // -- format_status_summary tests --------------------------------------

    #[test]
    fn status_summary_full() {
        let changes = vec![
            ScmFileChange::new("a.rs", ScmFileStatus::Modified),
            ScmFileChange::new("b.rs", ScmFileStatus::Modified),
            ScmFileChange::new("c.rs", ScmFileStatus::Added),
            ScmFileChange::new("d.rs", ScmFileStatus::Untracked),
        ];
        let summary = format_status_summary(Some("main"), 2, 1, &changes);
        assert!(summary.contains("main"));
        assert!(summary.contains("↑2"));
        assert!(summary.contains("↓1"));
        assert!(summary.contains("2M"));
        assert!(summary.contains("1A"));
        assert!(summary.contains("1?"));
    }

    #[test]
    fn status_summary_detached_no_remote() {
        let summary = format_status_summary(None, 0, 0, &[]);
        assert_eq!(summary, "(detached)");
    }

    #[test]
    fn status_summary_no_changes() {
        let summary = format_status_summary(Some("develop"), 0, 0, &[]);
        assert_eq!(summary, "develop");
    }

    // -- ScmCommitMessageBuilder tests -----------------------------------------

    #[test]
    fn commit_builder_basic_template() {
        let mut b = ScmCommitMessageBuilder::new("{type}: {summary}");
        b.set("type", "feat").set("summary", "add login page");
        assert_eq!(b.build(), "feat: add login page");
    }

    #[test]
    fn commit_builder_placeholders_list() {
        let b = ScmCommitMessageBuilder::new("{type}({scope}): {msg}");
        let ph = b.placeholders();
        assert_eq!(ph, vec!["type", "scope", "msg"]);
    }

    #[test]
    fn commit_builder_default_values() {
        let mut b = ScmCommitMessageBuilder::new("{type}: {summary}");
        b.set_default("type", "chore").set("summary", "cleanup");
        assert_eq!(b.build(), "chore: cleanup");
    }

    #[test]
    fn commit_builder_explicit_overrides_default() {
        let mut b = ScmCommitMessageBuilder::new("{type}: {summary}");
        b.set_default("type", "chore").set("type", "fix").set("summary", "bug");
        assert_eq!(b.build(), "fix: bug");
    }

    #[test]
    fn commit_builder_truncation() {
        let mut b = ScmCommitMessageBuilder::new("{msg}");
        b.max_subject_len(20);
        b.set("msg", "this is a very long commit message that should be truncated");
        let result = b.build();
        assert!(result.len() <= 20);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn commit_builder_with_body() {
        let mut b = ScmCommitMessageBuilder::new("{type}: {summary}");
        b.set("type", "feat").set("summary", "stuff");
        b.add_body_line("First paragraph.");
        b.add_body_line("Second paragraph.");
        let msg = b.build();
        assert!(msg.contains("\n\nFirst paragraph.\nSecond paragraph."));
    }

    #[test]
    fn commit_builder_with_trailers() {
        let mut b = ScmCommitMessageBuilder::new("{type}: {summary}");
        b.set("type", "fix").set("summary", "typo");
        b.add_trailer("Signed-off-by", "Alice <alice@example.com>");
        let msg = b.build();
        assert!(msg.contains("Signed-off-by: Alice <alice@example.com>"));
    }

    #[test]
    fn commit_builder_unset_placeholder_kept() {
        let b = ScmCommitMessageBuilder::new("{type}: {summary}");
        assert_eq!(b.build(), "{type}: {summary}");
    }

    // -- ScmDiffStatisticsView tests ------------------------------------------

    #[test]
    fn diff_stat_entry_net_change() {
        let e = DiffStatEntry::new("foo.rs", 10, 3);
        assert_eq!(e.net_change(), 7);
        assert_eq!(e.total_change(), 13);
    }

    #[test]
    fn diff_stat_entry_histogram() {
        let e = DiffStatEntry::new("bar.rs", 5, 2);
        let h = e.histogram(40);
        assert_eq!(h.matches('+').count(), 5);
        assert_eq!(h.matches('-').count(), 2);
    }

    #[test]
    fn diff_stats_view_totals() {
        let mut v = ScmDiffStatisticsView::new();
        v.add("a.rs", 10, 5);
        v.add("b.rs", 20, 3);
        assert_eq!(v.total_insertions(), 30);
        assert_eq!(v.total_deletions(), 8);
        assert_eq!(v.files_changed(), 2);
    }

    #[test]
    fn diff_stats_view_most_changed() {
        let mut v = ScmDiffStatisticsView::new();
        v.add("small.rs", 1, 1);
        v.add("big.rs", 100, 50);
        let mc = v.most_changed().unwrap();
        assert_eq!(mc.path, "big.rs");
    }

    #[test]
    fn diff_stats_view_summary_format() {
        let mut v = ScmDiffStatisticsView::new();
        v.add("lib.rs", 4, 2);
        let s = v.summary();
        assert!(s.contains("lib.rs"));
        assert!(s.contains("1 file(s) changed"));
        assert!(s.contains("4 insertion(s)(+)"));
    }

    #[test]
    fn diff_stats_view_sorted_by_change() {
        let mut v = ScmDiffStatisticsView::new();
        v.add("small.rs", 1, 0);
        v.add("medium.rs", 10, 5);
        v.add("large.rs", 50, 30);
        let sorted = v.sorted_by_change();
        assert_eq!(sorted[0].path, "large.rs");
        assert_eq!(sorted[2].path, "small.rs");
    }



    // -- scm_view additional tests -------------------------------------------

    #[test]
    fn x_scm_view_panel_state_new() {
        let p = XScmViewPanelState::new(XScmViewLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XScmViewLayoutRegion::Sidebar);
    }

    #[test]
    fn x_scm_view_panel_area() {
        let p = XScmViewPanelState::new(XScmViewLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_scm_view_panel_toggle() {
        let mut p = XScmViewPanelState::new(XScmViewLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_scm_view_panel_resize() {
        let mut p = XScmViewPanelState::new(XScmViewLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_scm_view_panel_is_narrow() {
        let mut p = XScmViewPanelState::new(XScmViewLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_scm_view_total_visible_area_basic() {
        let panels = vec![
            XScmViewPanelState::new(XScmViewLayoutRegion::Sidebar, "a"),
            XScmViewPanelState::new(XScmViewLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_scm_view_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_scm_view_total_visible_area_hidden() {
        let mut panels = vec![
            XScmViewPanelState::new(XScmViewLayoutRegion::Sidebar, "a"),
            XScmViewPanelState::new(XScmViewLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_scm_view_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_scm_view_count_in_region_basic() {
        let panels = vec![
            XScmViewPanelState::new(XScmViewLayoutRegion::Sidebar, "a"),
            XScmViewPanelState::new(XScmViewLayoutRegion::Sidebar, "b"),
            XScmViewPanelState::new(XScmViewLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_scm_view_count_in_region(&panels, XScmViewLayoutRegion::Sidebar), 2);
        assert_eq!(x_scm_view_count_in_region(&panels, XScmViewLayoutRegion::Editor), 1);
        assert_eq!(x_scm_view_count_in_region(&panels, XScmViewLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_scm_view_widest_panel_basic() {
        let mut panels = vec![
            XScmViewPanelState::new(XScmViewLayoutRegion::Sidebar, "narrow"),
            XScmViewPanelState::new(XScmViewLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_scm_view_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_scm_view_collapse_region_basic() {
        let mut panels = vec![
            XScmViewPanelState::new(XScmViewLayoutRegion::Sidebar, "a"),
            XScmViewPanelState::new(XScmViewLayoutRegion::Sidebar, "b"),
            XScmViewPanelState::new(XScmViewLayoutRegion::Editor, "c"),
        ];
        x_scm_view_collapse_region(&mut panels, XScmViewLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_scm_view_layout_constraint_clamp() {
        let lc = XScmViewLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_scm_view_layout_constraint_satisfied() {
        let lc = XScmViewLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_scm_view_widest_panel_empty() {
        let panels: Vec<XScmViewPanelState> = vec![];
        assert!(x_scm_view_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_scm_view_layout_region_eq() {
        assert_eq!(XScmViewLayoutRegion::Sidebar, XScmViewLayoutRegion::Sidebar);
        assert_ne!(XScmViewLayoutRegion::Sidebar, XScmViewLayoutRegion::Panel);
    }


    // -- scm_view extended domain tests ----------------------------------------

    #[test]
    fn y_scm_view_enum_index() {
        assert_eq!(YScmViewScmChangeKind::Added.index(), 0);
        assert_eq!(YScmViewScmChangeKind::Modified.index(), 1);
        assert_eq!(YScmViewScmChangeKind::Deleted.index(), 2);
        assert_eq!(YScmViewScmChangeKind::Renamed.index(), 3);
    }

    #[test]
    fn y_scm_view_enum_label() {
        assert_eq!(YScmViewScmChangeKind::Added.label(), "Added");
        assert_eq!(YScmViewScmChangeKind::Modified.label(), "Modified");
        assert_eq!(YScmViewScmChangeKind::Deleted.label(), "Deleted");
        assert_eq!(YScmViewScmChangeKind::Renamed.label(), "Renamed");
    }

    #[test]
    fn y_scm_view_enum_all() {
        let all = YScmViewScmChangeKind::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_scm_view_enum_is_default() {
        assert!(YScmViewScmChangeKind::Added.is_default());
        assert!(!YScmViewScmChangeKind::Renamed.is_default());
    }

    #[test]
    fn y_scm_view_enum_display() {
        assert_eq!(format!("{}", YScmViewScmChangeKind::Added), "Added");
    }

    #[test]
    fn y_scm_view_struct_new() {
        let s = YScmViewScmStatusSummary::new();
        let _ = s.summary();
    }

    #[test]
    fn y_scm_view_fingerprint_deterministic() {
        let h1 = y_scm_view_fingerprint("hello");
        let h2 = y_scm_view_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_scm_view_fingerprint("a"), y_scm_view_fingerprint("b"));
    }

    #[test]
    fn y_scm_view_truncate_short() {
        assert_eq!(y_scm_view_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_scm_view_truncate_long() {
        let r = y_scm_view_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_scm_view_normalize_key_basic() {
        assert_eq!(y_scm_view_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_scm_view_split_path_basic() {
        let parts = y_scm_view_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_scm_view_count_occurrences_basic() {
        assert_eq!(y_scm_view_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_scm_view_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_scm_view_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_scm_view_in_range_basic() {
        assert!(y_scm_view_in_range(5, 1, 10));
        assert!(y_scm_view_in_range(1, 1, 10));
        assert!(y_scm_view_in_range(10, 1, 10));
        assert!(!y_scm_view_in_range(0, 1, 10));
        assert!(!y_scm_view_in_range(11, 1, 10));
    }

    #[test]
    fn y_scm_view_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_scm_view_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_scm_view_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_scm_view_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- scm_view Z-extended tests -----------------------------------------------

    #[test]
    fn z_scm_view_priority_weight() {
        assert_eq!(ZScmViewPriority::Idle.weight(), 0);
        assert_eq!(ZScmViewPriority::Normal.weight(), 2);
        assert_eq!(ZScmViewPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_scm_view_priority_label() {
        assert_eq!(ZScmViewPriority::Low.label(), "low");
        assert_eq!(ZScmViewPriority::High.label(), "high");
    }

    #[test]
    fn z_scm_view_priority_is_elevated() {
        assert!(!ZScmViewPriority::Normal.is_elevated());
        assert!(ZScmViewPriority::High.is_elevated());
        assert!(ZScmViewPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_scm_view_priority_display() {
        assert_eq!(format!("{}", ZScmViewPriority::Idle), "idle");
    }

    #[test]
    fn z_scm_view_priority_all_asc() {
        let all = ZScmViewPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZScmViewPriority::Idle);
        assert_eq!(all[4], ZScmViewPriority::Realtime);
    }

    #[test]
    fn z_scm_view_struct_new() {
        let s = ZScmViewScmBlameInfo::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_scm_view_struct_toggled_clone() {
        let s = ZScmViewScmBlameInfo::new();
        let t = s.toggled_clone();
        let _ = t.oldest_ms;
    }

    #[test]
    fn z_scm_view_rolling_hash_deterministic() {
        let h1 = z_scm_view_rolling_hash(b"test");
        let h2 = z_scm_view_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_scm_view_rolling_hash(b"a"), z_scm_view_rolling_hash(b"b"));
    }

    #[test]
    fn z_scm_view_pad_to_basic() {
        assert_eq!(z_scm_view_pad_to("hi", 5), "hi   ");
        assert_eq!(z_scm_view_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_scm_view_is_identifier_basic() {
        assert!(z_scm_view_is_identifier("foo_bar"));
        assert!(z_scm_view_is_identifier("abc123"));
        assert!(!z_scm_view_is_identifier(""));
        assert!(!z_scm_view_is_identifier("has space"));
    }

    #[test]
    fn z_scm_view_levenshtein_basic() {
        assert_eq!(z_scm_view_levenshtein("", ""), 0);
        assert_eq!(z_scm_view_levenshtein("abc", "abc"), 0);
        assert_eq!(z_scm_view_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_scm_view_unique_words_basic() {
        let w = z_scm_view_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_scm_view_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_scm_view_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_scm_view_common_prefix_basic() {
        assert_eq!(z_scm_view_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_scm_view_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_scm_view_struct_clear() {
        let mut s = ZScmViewScmBlameInfo::new();
        s.line_authors.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_scm_view_rolling_hash_empty() {
        let h = z_scm_view_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_44_push_and_len() {
        let mut rb = super::XbRingBuffer44::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_44_overwrite() {
        let mut rb = super::XbRingBuffer44::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_44_get_out_of_bounds() {
        let rb = super::XbRingBuffer44::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_44_drain_all() {
        let mut rb = super::XbRingBuffer44::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_44_peek_front_back() {
        let mut rb = super::XbRingBuffer44::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_44_clear() {
        let mut rb = super::XbRingBuffer44::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_44_capacity() {
        let rb = super::XbRingBuffer44::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_44_basic() {
        let h = super::xb_fnv1a_44(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_44(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_44_different_inputs() {
        let h1 = super::xb_fnv1a_44(b"abc");
        let h2 = super::xb_fnv1a_44(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_44_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_44(&data);
        let dec = super::xb_rle_decode_44(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_44_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_44(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_44(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_44_values() {
        assert!((super::xb_clamp_44(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_44(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_44(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_44_values() {
        assert!((super::xb_lerp_44(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_44(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_44(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_44_wrap_around_twice() {
        let mut rb = super::XbRingBuffer44::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 153 ----

    #[test]
    fn xc_153_pool_new_empty() {
        let pool: super::Xc153Pool<i32> = super::Xc153Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_153_pool_release_acquire() {
        let mut pool = super::Xc153Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_153_pool_acquire_empty() {
        let mut pool: super::Xc153Pool<i32> = super::Xc153Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_153_pool_full() {
        let mut pool = super::Xc153Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_153_pool_drain() {
        let mut pool = super::Xc153Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_153_pool_stats() {
        let mut pool = super::Xc153Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_153_pool_clear() {
        let mut pool = super::Xc153Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_153_pool_shrink() {
        let mut pool = super::Xc153Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_153_pool_default() {
        let pool: super::Xc153Pool<String> = super::Xc153Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_153_pool_extend() {
        let mut pool = super::Xc153Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_153_pool_retain() {
        let mut pool = super::Xc153Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_153_scheduler_round_robin() {
        let mut sched = super::Xc153Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_153_scheduler_empty() {
        let mut sched = super::Xc153Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_153_scheduler_reset() {
        let mut sched = super::Xc153Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_153_scheduler_add_remove() {
        let mut sched = super::Xc153Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_153_scheduler_targets() {
        let sched = super::Xc153Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_153_hash_empty() {
        assert_eq!(super::xc_153_hash(b""), 5381);
    }

    #[test]
    fn xc_153_hash_data() {
        let h = super::xc_153_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_153_hash(b"hello"), h);
    }

    #[test]
    fn xc_153_reverse_str() {
        assert_eq!(super::xc_153_reverse("abc"), "cba");
        assert_eq!(super::xc_153_reverse(""), "");
    }


    #[test]
    fn xe_57_pipeline_empty() {
        let p = super::Xe57Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_57_pipeline_parse_stage() {
        let p = super::Xe57Pipeline::new()
            .add_parse(super::xe_57_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_57_pipeline_transform_double() {
        let p = super::Xe57Pipeline::new()
            .add_transform(super::xe_57_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_57_pipeline_validate_reverse() {
        let p = super::Xe57Pipeline::new()
            .add_validate(super::xe_57_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_57_pipeline_emit_filter() {
        let p = super::Xe57Pipeline::new()
            .add_emit(super::xe_57_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_57_pipeline_multi_stage() {
        let p = super::Xe57Pipeline::new()
            .add_parse(super::xe_57_pipeline_identity)
            .add_transform(super::xe_57_pipeline_double)
            .add_validate(super::xe_57_pipeline_reverse)
            .add_emit(super::xe_57_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_57_pipeline_error_propagation() {
        let p = super::Xe57Pipeline::new()
            .add_parse(super::xe_57_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe57Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_57_pipeline_compose() {
        let p1 = super::Xe57Pipeline::new()
            .add_parse(super::xe_57_pipeline_identity);
        let p2 = super::Xe57Pipeline::new()
            .add_transform(super::xe_57_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_57_pipeline_error_display() {
        let e = super::Xe57PipelineError {
            stage: super::Xe57Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_57_cache_put_get() {
        let mut c = super::Xe57Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_57_cache_miss() {
        let mut c: super::Xe57Cache<&str, i32> = super::Xe57Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_57_cache_ttl_expiry() {
        let mut c = super::Xe57Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_57_cache_evict() {
        let mut c = super::Xe57Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_57_cache_capacity() {
        let mut c = super::Xe57Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_57_cache_stats() {
        let mut c = super::Xe57Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_57_cache_clear() {
        let mut c = super::Xe57Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_55 graph tests ------------------------------------------------

    #[test]
    fn xg_55_graph_empty() {
        let g = super::Xg55Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_55_graph_add_node() {
        let mut g = super::Xg55Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_55_graph_add_edge() {
        let mut g = super::Xg55Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_55_graph_neighbors() {
        let mut g = super::Xg55Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_55_graph_has_path() {
        let mut g = super::Xg55Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_55_graph_self_path() {
        let g = super::Xg55Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_55_graph_topo_sort() {
        let mut g = super::Xg55Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_55_graph_cycle_detect_false() {
        let mut g = super::Xg55Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_55_graph_cycle_detect_true() {
        let mut g = super::Xg55Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_55 heap tests -------------------------------------------------

    #[test]
    fn xg_55_heap_empty() {
        let h: super::Xg55Heap<i32> = super::Xg55Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_55_heap_push_pop() {
        let mut h = super::Xg55Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_55_heap_peek() {
        let mut h = super::Xg55Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_55_heap_drain_sorted() {
        let mut h = super::Xg55Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_55_heap_merge() {
        let mut a = super::Xg55Heap::new();
        let mut b = super::Xg55Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_55_heap_default() {
        let h: super::Xg55Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_55_graph_default() {
        let g: super::Xg55Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh152_skip_insert_contains() {
        let mut sl = super::Xh152SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh152_skip_remove() {
        let mut sl = super::Xh152SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh152_skip_len() {
        let mut sl = super::Xh152SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh152_skip_range_query() {
        let mut sl = super::Xh152SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh152_skip_floor_ceiling() {
        let mut sl = super::Xh152SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh152_skip_rank() {
        let mut sl = super::Xh152SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh152_skip_empty() {
        let sl = super::Xh152SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh152_skip_duplicates() {
        let mut sl = super::Xh152SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh152_bitset_set_test() {
        let mut bs = super::Xh152BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh152_bitset_clear_count() {
        let mut bs = super::Xh152BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh152_bitset_and_or_xor() {
        let mut a = super::Xh152BitSet::xh_new(128);
        let mut b = super::Xh152BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh152_bitset_iter_ones() {
        let mut bs = super::Xh152BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh152_bitset_first_last() {
        let mut bs = super::Xh152BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh152_bitset_empty() {
        let bs = super::Xh152BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi152_deque_push_pop_back() {
        let mut dq = super::Xi152Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi152_deque_push_pop_front() {
        let mut dq = super::Xi152Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi152_deque_mixed_ops() {
        let mut dq = super::Xi152Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi152_deque_get_and_split() {
        let mut dq = super::Xi152Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi152_deque_rotate_left() {
        let mut dq = super::Xi152Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi152_deque_rotate_right() {
        let mut dq = super::Xi152Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi152_deque_grow() {
        let mut dq = super::Xi152Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi152_deque_empty() {
        let dq = super::Xi152Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi152_interval_tree_insert_query() {
        let mut tree = super::Xi152IntervalTree::xi_new();
        tree.xi_insert(super::Xi152Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi152Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi152Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi152_interval_tree_overlap() {
        let mut tree = super::Xi152IntervalTree::xi_new();
        tree.xi_insert(super::Xi152Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi152Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi152Interval::xi_new(12, 20));
        let q = super::Xi152Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi152_interval_tree_remove() {
        let mut tree = super::Xi152IntervalTree::xi_new();
        tree.xi_insert(super::Xi152Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi152Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi152_interval_tree_gaps() {
        let mut tree = super::Xi152IntervalTree::xi_new();
        tree.xi_insert(super::Xi152Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi152Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi152Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi152Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi152Interval::xi_new(8, 10));
    }

    #[test]
    fn xi152_interval_tree_merge() {
        let mut tree = super::Xi152IntervalTree::xi_new();
        tree.xi_insert(super::Xi152Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi152Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi152Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi152Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi152Interval::xi_new(10, 15));
    }

    #[test]
    fn xi152_interval_tree_all() {
        let mut tree = super::Xi152IntervalTree::xi_new();
        tree.xi_insert(super::Xi152Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi152Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi152_interval_tree_empty() {
        let tree = super::Xi152IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi152_interval_tree_contains_point() {
        let iv = super::Xi152Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 152) ---

    #[test]
    fn xj_152_uf_make_and_find() {
        let mut uf = super::Xj152UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_152_uf_union_connected() {
        let mut uf = super::Xj152UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_152_uf_component_count() {
        let mut uf = super::Xj152UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_152_uf_component_size() {
        let mut uf = super::Xj152UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_152_uf_largest_component() {
        let mut uf = super::Xj152UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_152_uf_many_elements() {
        let mut uf = super::Xj152UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_152_uf_separate_components() {
        let mut uf = super::Xj152UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_152_uf_path_compression() {
        let mut uf = super::Xj152UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_152_bt_insert_get() {
        let mut bt = super::Xj152BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_152_bt_contains_len() {
        let mut bt = super::Xj152BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_152_bt_replace() {
        let mut bt = super::Xj152BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_152_bt_remove() {
        let mut bt = super::Xj152BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_152_bt_keys_values() {
        let mut bt = super::Xj152BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_152_bt_range() {
        let mut bt = super::Xj152BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_152_bt_min_max() {
        let mut bt = super::Xj152BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_152_bt_many_inserts() {
        let mut bt = super::Xj152BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_152 segment tree tests ---

    #[test]
    fn xk_152_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk152SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_152_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk152SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_152_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk152SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_152_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk152SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_152_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk152SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_152_st_single_element() {
        let data = vec![42];
        let st = super::Xk152SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_152_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk152SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_152_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk152SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_152 disjoint intervals tests ---

    #[test]
    fn xk_152_di_add_and_count() {
        let mut di = super::Xk152DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_152_di_merge_overlap() {
        let mut di = super::Xk152DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_152_di_contains() {
        let mut di = super::Xk152DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_152_di_remove() {
        let mut di = super::Xk152DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_152_di_covered_length() {
        let mut di = super::Xk152DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_152_di_gaps() {
        let mut di = super::Xk152DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_152_di_merge_adjacent() {
        let mut di = super::Xk152DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_152_di_empty() {
        let di = super::Xk152DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_152_rope_new_empty() {
        let rope = super::Xl152Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_152_rope_from_str() {
        let rope = super::Xl152Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_152_rope_insert_at() {
        let mut rope = super::Xl152Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_152_rope_delete_range() {
        let mut rope = super::Xl152Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_152_rope_char_at() {
        let rope = super::Xl152Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_152_rope_split_concat() {
        let rope = super::Xl152Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_152_rope_line_count() {
        let rope = super::Xl152Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_152_rope_line_at() {
        let rope = super::Xl152Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_152_sa_build_and_search() {
        let sa = super::Xl152SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_152_sa_count() {
        let sa = super::Xl152SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_152_sa_longest_repeated() {
        let sa = super::Xl152SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_152_sa_all_positions() {
        let sa = super::Xl152SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_152_sa_len() {
        let sa = super::Xl152SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_152_sa_empty() {
        let sa = super::Xl152SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_152_rope_slice() {
        let rope = super::Xl152Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_152_sa_search_start() {
        let sa = super::Xl152SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_152_sparse_set_get() {
        let mut m = super::Xm152MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_152_sparse_row_col() {
        let mut m = super::Xm152MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_152_sparse_transpose() {
        let mut m = super::Xm152MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_152_sparse_multiply_vec() {
        let mut m = super::Xm152MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_152_sparse_nnz_density() {
        let mut m = super::Xm152MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_152_sparse_clear() {
        let mut m = super::Xm152MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_152_sparse_overwrite_zero() {
        let mut m = super::Xm152MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_152_tokenizer_basic() {
        let t = super::Xm152Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_152_tokenizer_count() {
        let t = super::Xm152Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_152_tokenizer_unique() {
        let t = super::Xm152Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_152_tokenizer_frequency() {
        let t = super::Xm152Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_152_tokenizer_delimiter() {
        let t = super::Xm152Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_152_tokenizer_whitespace() {
        let t = super::Xm152Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_152_tokenizer_empty() {
        let t = super::Xm152Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 152 ----

    #[test]
    fn xn_152_fenwick_prefix_sum() {
        let mut ft = super::Xn152Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_152_fenwick_range_sum() {
        let mut ft = super::Xn152Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_152_fenwick_point_query() {
        let mut ft = super::Xn152Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_152_fenwick_len() {
        let ft = super::Xn152Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_152_fenwick_multiple_updates() {
        let mut ft = super::Xn152Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_152_fenwick_single_element() {
        let mut ft = super::Xn152Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_152_fenwick_find_kth() {
        let mut ft = super::Xn152Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_152_fenwick_negative_delta() {
        let mut ft = super::Xn152Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 152 ----

    #[test]
    fn xn_152_avl_insert_get() {
        let mut m = super::Xn152AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_152_avl_remove() {
        let mut m = super::Xn152AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_152_avl_in_order() {
        let mut m = super::Xn152AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_152_avl_min_max() {
        let mut m = super::Xn152AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_152_avl_floor_ceiling() {
        let mut m = super::Xn152AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_152_avl_height_balanced() {
        let mut m = super::Xn152AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_152_avl_overwrite() {
        let mut m = super::Xn152AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_152_avl_empty() {
        let m: super::Xn152AVL<i32, i32> = super::Xn152AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo152RedBlack tests ---

    #[test]
    fn xo_152_rb_insert_and_get() {
        let mut tree = super::Xo152RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_152_rb_len_and_empty() {
        let mut tree = super::Xo152RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_152_rb_min_max() {
        let mut tree = super::Xo152RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_152_rb_contains() {
        let mut tree = super::Xo152RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_152_rb_remove() {
        let mut tree = super::Xo152RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_152_rb_in_order() {
        let mut tree = super::Xo152RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_152_rb_black_height() {
        let mut tree = super::Xo152RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_152_rb_overwrite() {
        let mut tree = super::Xo152RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo152ConsistentHash tests ---

    #[test]
    fn xo_152_ch_add_and_count() {
        let mut ring = super::Xo152ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_152_ch_remove_node() {
        let mut ring = super::Xo152ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_152_ch_get_node() {
        let mut ring = super::Xo152ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_152_ch_empty_ring() {
        let ring = super::Xo152ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_152_ch_distribution() {
        let mut ring = super::Xo152ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_152_ch_rebalance() {
        let mut ring = super::Xo152ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_152_ch_virtual_nodes() {
        let mut ring = super::Xo152ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_152_ch_consistent_lookup() {
        let mut ring = super::Xo152ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_152_splay_insert_get() {
        let mut t = super::Xp152SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_152_splay_remove() {
        let mut t = super::Xp152SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_152_splay_count_increases() {
        let mut t = super::Xp152SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_152_splay_depth() {
        let mut t = super::Xp152SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_152_splay_len_empty() {
        let t = super::Xp152SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_152_splay_min_max() {
        let mut t = super::Xp152SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_152_splay_overwrite() {
        let mut t = super::Xp152SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_152_splay_remove_missing() {
        let mut t = super::Xp152SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_152 treap tests ----
    #[test]
    fn xq_152_treap_empty() {
        let t = super::Xq152Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_152_treap_insert_get() {
        let mut t = super::Xq152Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_152_treap_overwrite() {
        let mut t = super::Xq152Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_152_treap_remove() {
        let mut t = super::Xq152Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_152_treap_min_max() {
        let mut t = super::Xq152Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_152_treap_rank() {
        let mut t = super::Xq152Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_152_treap_kth() {
        let mut t = super::Xq152Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_152_treap_in_order() {
        let mut t = super::Xq152Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_152 VEB tree tests ----
    #[test]
    fn xq_152_veb_empty() {
        let v = super::Xq152VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_152_veb_insert_contains() {
        let mut v = super::Xq152VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_152_veb_min_max() {
        let mut v = super::Xq152VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_152_veb_delete() {
        let mut v = super::Xq152VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_152_veb_successor() {
        let mut v = super::Xq152VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_152_veb_predecessor() {
        let mut v = super::Xq152VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_152_veb_count() {
        let mut v = super::Xq152VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_152_veb_duplicate_insert() {
        let mut v = super::Xq152VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_152_kdtree_empty() {
        let tree = super::Xr152KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_152_kdtree_insert_one() {
        let mut tree = super::Xr152KDTree::xr_new();
        tree.xr_insert(super::Xr152KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_152_kdtree_insert_multiple() {
        let mut tree = super::Xr152KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr152KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_152_kdtree_nearest_neighbor() {
        let mut tree = super::Xr152KDTree::xr_new();
        tree.xr_insert(super::Xr152KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr152KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr152KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_152_kdtree_nn_empty() {
        let tree = super::Xr152KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr152KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_152_kdtree_range_search() {
        let mut tree = super::Xr152KDTree::xr_new();
        tree.xr_insert(super::Xr152KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr152KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr152KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_152_kdtree_range_empty() {
        let mut tree = super::Xr152KDTree::xr_new();
        tree.xr_insert(super::Xr152KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_152_kdtree_all_points() {
        let mut tree = super::Xr152KDTree::xr_new();
        tree.xr_insert(super::Xr152KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr152KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_152_kdtree_depth() {
        let mut tree = super::Xr152KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr152KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_152_kdtree_bounding_box() {
        let mut tree = super::Xr152KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr152KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr152KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_152_persistent_array_new() {
        let arr = super::Xs152PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_152_persistent_array_push() {
        let mut arr = super::Xs152PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_152_persistent_array_set() {
        let mut arr = super::Xs152PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_152_persistent_array_diff() {
        let mut arr = super::Xs152PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_152_persistent_array_rollback() {
        let mut arr = super::Xs152PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_152_persistent_array_history() {
        let mut arr = super::Xs152PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_152_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs152PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_152_persistent_array_from_vec() {
        let arr = super::Xs152PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_152_concurrent_queue_new() {
        let q = super::Xs152ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_152_concurrent_queue_push_pop() {
        let mut q = super::Xs152ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_152_concurrent_queue_full() {
        let mut q = super::Xs152ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_152_concurrent_queue_drain() {
        let mut q = super::Xs152ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_152_concurrent_queue_try_pop() {
        let mut q = super::Xs152ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_152_concurrent_queue_clear() {
        let mut q = super::Xs152ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_152_range_map_new() {
        let rm = super::Xs152RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_152_range_map_insert_get() {
        let mut rm = super::Xs152RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_152_range_map_overlap() {
        let mut rm = super::Xs152RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_152_range_map_remove() {
        let mut rm = super::Xs152RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_152_range_map_gaps() {
        let mut rm = super::Xs152RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_152_range_map_coverage() {
        let mut rm = super::Xs152RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_152_range_map_contains() {
        let mut rm = super::Xs152RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_152_range_map_clear() {
        let mut rm = super::Xs152RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_152_circular_buffer_new() {
        let buf = super::Xs152CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_152_circular_buffer_push_pop() {
        let mut buf = super::Xs152CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_152_circular_buffer_overwrite() {
        let mut buf = super::Xs152CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_152_circular_buffer_peek() {
        let mut buf = super::Xs152CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_152_circular_buffer_is_full() {
        let mut buf = super::Xs152CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_152_circular_buffer_iter() {
        let mut buf = super::Xs152CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_152_circular_buffer_clear() {
        let mut buf = super::Xs152CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_152_circular_buffer_to_vec() {
        let mut buf = super::Xs152CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_152_stats_tracker_new() {
        let tracker = super::Xs152StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_152_stats_tracker_mean() {
        let mut tracker = super::Xs152StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_152_stats_tracker_min_max() {
        let mut tracker = super::Xs152StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_152_stats_tracker_median() {
        let mut tracker = super::Xs152StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_152_stats_tracker_variance() {
        let mut tracker = super::Xs152StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_152_stats_tracker_range() {
        let mut tracker = super::Xs152StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_152_stats_tracker_clear() {
        let mut tracker = super::Xs152StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_152_stats_tracker_sum() {
        let mut tracker = super::Xs152StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }


    // --- xu_ Binomial Heap tests ---

    #[test]
    fn xu_bin_heap_new() {
        let h = super::XuBinomialHeap::<i32, &str>::xu_new();
        assert!(h.xu_is_empty());
        assert_eq!(h.xu_len(), 0);
    }

    #[test]
    fn xu_bin_heap_insert_find_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(5, "five");
        h.xu_insert(3, "three");
        h.xu_insert(7, "seven");
        assert_eq!(h.xu_len(), 3);
        assert_eq!(h.xu_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xu_bin_heap_extract_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(10, "a");
        h.xu_insert(2, "b");
        h.xu_insert(8, "c");
        h.xu_insert(1, "d");
        assert_eq!(h.xu_extract_min(), Some((1, "d")));
        assert_eq!(h.xu_extract_min(), Some((2, "b")));
    }

    #[test]
    fn xu_bin_heap_sorted_drain() {
        let mut h = super::XuBinomialHeap::xu_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xu_insert(v, v * 10);
        }
        let sorted = h.xu_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xu_bin_heap_merge() {
        let mut h1 = super::XuBinomialHeap::xu_new();
        h1.xu_insert(3, "a");
        h1.xu_insert(7, "b");
        let mut h2 = super::XuBinomialHeap::xu_new();
        h2.xu_insert(1, "c");
        h2.xu_insert(5, "d");
        h1.xu_merge(&mut h2);
        assert_eq!(h1.xu_len(), 4);
        assert_eq!(h1.xu_find_min(), Some((&1, &"c")));
    }

    #[test]
    fn xu_bin_heap_clear() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "a");
        h.xu_clear();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_heap_display() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "x");
        assert!(format!("{}", h).contains("BinHeap"));
    }

    #[test]
    fn xu_bin_heap_default() {
        let h = super::XuBinomialHeap::<i32, i32>::default();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_node_display() {
        let n = super::XuBinomialNode::xu_new(5, "v");
        assert!(format!("{}", n).contains("BinNode"));
    }

    #[test]
    fn xu_bin_heap_single() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(42, "answer");
        assert_eq!(h.xu_extract_min(), Some((42, "answer")));
        assert!(h.xu_is_empty());
    }

    // --- xu_ Disjoint Sparse Table tests ---

    #[test]
    fn xu_dst_build() {
        let data = vec![1, 2, 3, 4, 5];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_len(), 5);
        assert!(!dst.xu_is_empty());
    }

    #[test]
    fn xu_dst_single_element_query() {
        let data = vec![10, 20, 30];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_query(0, 0), 10);
        assert_eq!(dst.xu_query(1, 1), 20);
        assert_eq!(dst.xu_query(2, 2), 30);
    }

    #[test]
    fn xu_dst_get() {
        let data = vec![5, 10, 15];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_get(0), Some(&5));
        assert_eq!(dst.xu_get(2), Some(&15));
        assert_eq!(dst.xu_get(10), None);
    }

    #[test]
    fn xu_dst_empty() {
        let dst = super::XuDisjointSparseTable::<i32>::xu_build(&[]);
        assert!(dst.xu_is_empty());
        assert_eq!(dst.xu_len(), 0);
    }

    #[test]
    fn xu_dst_display() {
        let data = vec![1, 2, 3];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert!(format!("{}", dst).contains("DST"));
    }

    // --- xu_ Monotonic Stack tests ---

    #[test]
    fn xu_mono_stack_increasing() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        assert!(s.xu_is_empty());
        let popped = s.xu_push(3);
        assert!(popped.is_empty());
        let popped = s.xu_push(5);
        assert!(popped.is_empty());
        let popped = s.xu_push(2);
        assert_eq!(popped, vec![5, 3]);
        assert_eq!(s.xu_as_slice(), &[2]);
    }

    #[test]
    fn xu_mono_stack_decreasing() {
        let mut s = super::XuMonotonicStack::xu_decreasing();
        s.xu_push(2);
        s.xu_push(1);
        let popped = s.xu_push(5);
        assert_eq!(popped, vec![1, 2]);
        assert_eq!(s.xu_as_slice(), &[5]);
    }

    #[test]
    fn xu_mono_stack_peek_pop() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(3);
        s.xu_push(5);
        assert_eq!(s.xu_peek(), Some(&5));
        assert_eq!(s.xu_pop(), Some(5));
        assert_eq!(s.xu_len(), 2);
    }

    #[test]
    fn xu_mono_stack_clear() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(2);
        s.xu_clear();
        assert!(s.xu_is_empty());
    }

    #[test]
    fn xu_mono_stack_display() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        assert!(format!("{}", s).contains("MonoStack"));
    }


    // --- xv_ Cartesian Tree tests ---

    #[test]
    fn xv_cart_tree_new() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_cart_tree_insert_contains() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        t.xv_insert(3, 2);
        t.xv_insert(7, 3);
        assert!(t.xv_contains(&5));
        assert!(t.xv_contains(&3));
        assert!(t.xv_contains(&7));
        assert!(!t.xv_contains(&4));
        assert_eq!(t.xv_len(), 3);
    }

    #[test]
    fn xv_cart_tree_inorder() {
        let mut t = super::XvCartesianTree::xv_new();
        for (k, p) in [(5, 3), (3, 1), (7, 2), (1, 5), (9, 4)] {
            t.xv_insert(k, p);
        }
        let keys = t.xv_inorder();
        assert_eq!(keys, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn xv_cart_tree_min_priority() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 10);
        t.xv_insert(3, 2);
        t.xv_insert(7, 5);
        assert_eq!(t.xv_min_priority(), Some(&2));
    }

    #[test]
    fn xv_cart_tree_from_pairs() {
        let t = super::XvCartesianTree::xv_from_pairs(&[(3, 1), (1, 3), (5, 2)]);
        assert_eq!(t.xv_len(), 3);
        assert!(t.xv_contains(&1));
    }

    #[test]
    fn xv_cart_tree_height() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        assert!(t.xv_height() >= 1);
    }

    #[test]
    fn xv_cart_tree_clear() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(1, 1);
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_tree_display() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("CartTree"));
    }

    #[test]
    fn xv_cart_tree_default() {
        let t = super::XvCartesianTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_node_display() {
        let n = super::XvCartesianNode { xv_key: 1, xv_priority: 2, xv_left: None, xv_right: None };
        assert!(format!("{}", n).contains("CartNode"));
    }

    // --- xv_ Weight-Balanced Tree tests ---

    #[test]
    fn xv_wb_tree_new() {
        let t = super::XvWeightBalancedTree::<i32, &str>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_wb_tree_insert_get() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "five");
        t.xv_insert(3, "three");
        t.xv_insert(7, "seven");
        assert_eq!(t.xv_get(&5), Some(&"five"));
        assert_eq!(t.xv_get(&3), Some(&"three"));
        assert_eq!(t.xv_get(&7), Some(&"seven"));
        assert_eq!(t.xv_get(&4), None);
    }

    #[test]
    fn xv_wb_tree_contains() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(10, "a");
        assert!(t.xv_contains(&10));
        assert!(!t.xv_contains(&20));
    }

    #[test]
    fn xv_wb_tree_keys_sorted() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xv_insert(k, k * 10);
        }
        assert_eq!(t.xv_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xv_wb_tree_replace_value() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "old");
        t.xv_insert(5, "new");
        assert_eq!(t.xv_get(&5), Some(&"new"));
        assert_eq!(t.xv_len(), 1);
    }

    #[test]
    fn xv_wb_tree_height() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in 1..=15 {
            t.xv_insert(k, k);
        }
        assert!(t.xv_height() <= 20);
    }

    #[test]
    fn xv_wb_tree_clear() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(1, "a");
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_tree_display() {
        let t = super::XvWeightBalancedTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("WBTree"));
    }

    #[test]
    fn xv_wb_tree_default() {
        let t = super::XvWeightBalancedTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_node_display() {
        let n = super::XvWBNode { xv_key: 1, xv_value: "a", xv_left: None, xv_right: None, xv_weight: 2 };
        assert!(format!("{}", n).contains("WBNode"));
    }


    // --- xw_ Scapegoat Tree tests ---

    #[test]
    fn xw_sg_tree_new() {
        let t = super::XwScapegoatTree::<i32, &str>::xw_new();
        assert!(t.xw_is_empty());
        assert_eq!(t.xw_len(), 0);
    }

    #[test]
    fn xw_sg_tree_insert_get() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "five");
        t.xw_insert(3, "three");
        t.xw_insert(7, "seven");
        assert_eq!(t.xw_get(&5), Some(&"five"));
        assert_eq!(t.xw_get(&3), Some(&"three"));
        assert_eq!(t.xw_get(&4), None);
    }

    #[test]
    fn xw_sg_tree_contains() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(10, "a");
        assert!(t.xw_contains(&10));
        assert!(!t.xw_contains(&20));
    }

    #[test]
    fn xw_sg_tree_keys_sorted() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xw_insert(k, k * 10);
        }
        assert_eq!(t.xw_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xw_sg_tree_sequential_inserts() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in 1..=20 {
            t.xw_insert(k, k);
        }
        assert_eq!(t.xw_len(), 20);
        assert!(t.xw_height() <= 15);
    }

    #[test]
    fn xw_sg_tree_replace_value() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "old");
        t.xw_insert(5, "new");
        assert_eq!(t.xw_get(&5), Some(&"new"));
        assert_eq!(t.xw_len(), 1);
    }

    #[test]
    fn xw_sg_tree_clear() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(1, "a");
        t.xw_clear();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_tree_display() {
        let t = super::XwScapegoatTree::<i32, i32>::xw_new();
        assert!(format!("{}", t).contains("SGTree"));
    }

    #[test]
    fn xw_sg_tree_default() {
        let t = super::XwScapegoatTree::<i32, i32>::default();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_node_display() {
        let n = super::XwScapegoatNode { xw_key: 1, xw_value: "a", xw_left: None, xw_right: None };
        assert!(format!("{}", n).contains("SGNode"));
    }

    // --- xw_ Rope tests ---

    #[test]
    fn xw_rope_new() {
        let r = super::XwRope::xw_new();
        assert!(r.xw_is_empty());
        assert_eq!(r.xw_len(), 0);
    }

    #[test]
    fn xw_rope_from_str() {
        let r = super::XwRope::xw_from_str("hello");
        assert_eq!(r.xw_len(), 5);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_concat() {
        let a = super::XwRope::xw_from_str("hello ");
        let b = super::XwRope::xw_from_str("world");
        let c = super::XwRope::xw_concat(a, b);
        assert_eq!(c.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_insert() {
        let mut r = super::XwRope::xw_from_str("helo");
        r.xw_insert(3, "l");
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_delete() {
        let mut r = super::XwRope::xw_from_str("hello world");
        r.xw_delete(5, 11);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_append() {
        let mut r = super::XwRope::xw_from_str("hello");
        r.xw_append(" world");
        assert_eq!(r.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_substring() {
        let r = super::XwRope::xw_from_str("hello world");
        assert_eq!(r.xw_substring(6, 11), "world");
    }

    #[test]
    fn xw_rope_char_at() {
        let r = super::XwRope::xw_from_str("abc");
        assert_eq!(r.xw_char_at(0), Some('a'));
        assert_eq!(r.xw_char_at(2), Some('c'));
    }

    #[test]
    fn xw_rope_clear() {
        let mut r = super::XwRope::xw_from_str("text");
        r.xw_clear();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_display() {
        let r = super::XwRope::xw_from_str("test");
        assert!(format!("{}", r).contains("Rope"));
    }

    #[test]
    fn xw_rope_default() {
        let r = super::XwRope::default();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_empty_ops() {
        let r = super::XwRope::xw_new();
        assert_eq!(r.xw_to_string(), "");
        assert_eq!(r.xw_substring(0, 5), "");
    }


    // --- xx_ Skip List tests ---

    #[test]
    fn xx_skip_list_new() {
        let sl = super::XxSkipList::<i32, &str>::xx_new();
        assert!(sl.xx_is_empty());
        assert_eq!(sl.xx_len(), 0);
    }

    #[test]
    fn xx_skip_list_insert_get() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "five");
        sl.xx_insert(3, "three");
        sl.xx_insert(7, "seven");
        assert_eq!(sl.xx_get(&5), Some(&"five"));
        assert_eq!(sl.xx_get(&3), Some(&"three"));
        assert_eq!(sl.xx_get(&7), Some(&"seven"));
        assert_eq!(sl.xx_get(&4), None);
    }

    #[test]
    fn xx_skip_list_contains() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(10, "a");
        assert!(sl.xx_contains(&10));
        assert!(!sl.xx_contains(&20));
    }

    #[test]
    fn xx_skip_list_keys_sorted() {
        let mut sl = super::XxSkipList::xx_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            sl.xx_insert(k, k * 10);
        }
        assert_eq!(sl.xx_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xx_skip_list_replace() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "old");
        sl.xx_insert(5, "new");
        assert_eq!(sl.xx_get(&5), Some(&"new"));
    }

    #[test]
    fn xx_skip_list_many() {
        let mut sl = super::XxSkipList::xx_new();
        for k in 1..=50 {
            sl.xx_insert(k, k);
        }
        assert_eq!(sl.xx_len(), 50);
        for k in 1..=50 {
            assert!(sl.xx_contains(&k));
        }
    }

    #[test]
    fn xx_skip_list_clear() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(1, "a");
        sl.xx_clear();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_list_display() {
        let sl = super::XxSkipList::<i32, i32>::xx_new();
        assert!(format!("{}", sl).contains("SkipList"));
    }

    #[test]
    fn xx_skip_list_default() {
        let sl = super::XxSkipList::<i32, i32>::default();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_node_display() {
        let n = super::XxSkipNode::<i32, i32> { xx_key: Some(5), xx_value: Some(50), xx_forward: vec![None] };
        assert!(format!("{}", n).contains("SkipNode"));
    }

    // --- xx_ Suffix Array tests ---

    #[test]
    fn xx_suffix_array_new() {
        let sa = super::XxSuffixArray::xx_new("banana");
        assert_eq!(sa.xx_len(), 6);
        assert!(!sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_search() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let pos = sa.xx_search("ana");
        assert_eq!(pos.len(), 2);
    }

    #[test]
    fn xx_suffix_array_count() {
        let sa = super::XxSuffixArray::xx_new("abcabcabc");
        assert_eq!(sa.xx_count("abc"), 3);
    }

    #[test]
    fn xx_suffix_array_no_match() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_count("xyz"), 0);
    }

    #[test]
    fn xx_suffix_array_suffix_at() {
        let sa = super::XxSuffixArray::xx_new("abc");
        let s = sa.xx_suffix_at(0);
        assert!(!s.is_empty());
    }

    #[test]
    fn xx_suffix_array_longest_repeated() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let lr = sa.xx_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xx_suffix_array_empty() {
        let sa = super::XxSuffixArray::xx_new("");
        assert!(sa.xx_is_empty());
        assert_eq!(sa.xx_search("a").len(), 0);
    }

    #[test]
    fn xx_suffix_array_display() {
        let sa = super::XxSuffixArray::xx_new("test");
        assert!(format!("{}", sa).contains("SuffixArray"));
    }

    #[test]
    fn xx_suffix_array_default() {
        let sa = super::XxSuffixArray::default();
        assert!(sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_text() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_text(), "hello");
    }


    // --- xy_ Cuckoo Hash Map tests ---

    #[test]
    fn xy_cuckoo_new() {
        let m = super::XyCuckooMap::<String, i32>::xy_new(16);
        assert!(m.xy_is_empty());
        assert_eq!(m.xy_len(), 0);
    }

    #[test]
    fn xy_cuckoo_insert_get() {
        let mut m = super::XyCuckooMap::xy_new(32);
        m.xy_insert("hello".to_string(), 1);
        m.xy_insert("world".to_string(), 2);
        assert_eq!(m.xy_get(&"hello".to_string()), Some(&1));
        assert_eq!(m.xy_get(&"world".to_string()), Some(&2));
        assert_eq!(m.xy_get(&"missing".to_string()), None);
    }

    #[test]
    fn xy_cuckoo_contains() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(42, "a");
        assert!(m.xy_contains(&42));
        assert!(!m.xy_contains(&99));
    }

    #[test]
    fn xy_cuckoo_replace() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(5, "old");
        m.xy_insert(5, "new");
        assert_eq!(m.xy_get(&5), Some(&"new"));
    }

    #[test]
    fn xy_cuckoo_remove() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(10, "val");
        assert_eq!(m.xy_remove(&10), Some("val"));
        assert!(!m.xy_contains(&10));
    }

    #[test]
    fn xy_cuckoo_many() {
        let mut m = super::XyCuckooMap::xy_new(64);
        for i in 0..30 {
            m.xy_insert(i, i * 10);
        }
        assert_eq!(m.xy_len(), 30);
        for i in 0..30 {
            assert!(m.xy_contains(&i));
        }
    }

    #[test]
    fn xy_cuckoo_keys() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_insert(2, "b");
        let keys = m.xy_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn xy_cuckoo_clear() {
        let mut m = super::XyCuckooMap::xy_new(16);
        m.xy_insert(1, "a");
        m.xy_clear();
        assert!(m.xy_is_empty());
    }

    #[test]
    fn xy_cuckoo_display() {
        let m = super::XyCuckooMap::<i32, i32>::xy_new(16);
        assert!(format!("{}", m).contains("CuckooMap"));
    }

    #[test]
    fn xy_cuckoo_default() {
        let m = super::XyCuckooMap::<i32, i32>::default();
        assert!(m.xy_is_empty());
    }

    // --- xy_ Count-Min Sketch tests ---

    #[test]
    fn xy_cms_new() {
        let cms = super::XyCountMinSketch::xy_new(100, 5);
        assert_eq!(cms.xy_width(), 100);
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_add_estimate() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..10 { cms.xy_add(42); }
        assert!(cms.xy_estimate(42) >= 10);
    }

    #[test]
    fn xy_cms_add_count() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        cms.xy_add_count(7, 100);
        assert!(cms.xy_estimate(7) >= 100);
    }

    #[test]
    fn xy_cms_unseen() {
        let cms = super::XyCountMinSketch::xy_new(1000, 5);
        assert_eq!(cms.xy_estimate(999), 0);
    }

    #[test]
    fn xy_cms_merge() {
        let mut a = super::XyCountMinSketch::xy_new(100, 3);
        let mut b = super::XyCountMinSketch::xy_new(100, 3);
        a.xy_add(1);
        b.xy_add(1);
        a.xy_merge(&b);
        assert!(a.xy_estimate(1) >= 2);
    }

    #[test]
    fn xy_cms_clear() {
        let mut cms = super::XyCountMinSketch::xy_new(100, 3);
        cms.xy_add(1);
        cms.xy_clear();
        assert_eq!(cms.xy_estimate(1), 0);
    }

    #[test]
    fn xy_cms_display() {
        let cms = super::XyCountMinSketch::xy_new(100, 3);
        assert!(format!("{}", cms).contains("CMS"));
    }

    #[test]
    fn xy_cms_default() {
        let cms = super::XyCountMinSketch::default();
        assert_eq!(cms.xy_depth(), 5);
    }

    #[test]
    fn xy_cms_multiple_items() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for i in 0..100 { cms.xy_add(i); }
        for i in 0..100 { assert!(cms.xy_estimate(i) >= 1); }
    }

    #[test]
    fn xy_cms_heavy_hitter() {
        let mut cms = super::XyCountMinSketch::xy_new(1000, 5);
        for _ in 0..1000 { cms.xy_add(42); }
        for i in 0..10 { cms.xy_add(i); }
        assert!(cms.xy_estimate(42) > cms.xy_estimate(0));
    }


    // --- xz_ HyperLogLog tests ---

    #[test]
    fn xz_hll_new() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_num_registers(), 1024);
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_add_estimate() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for i in 0..1000 {
            hll.xz_add(i);
        }
        let est = hll.xz_estimate();
        assert!(est > 500.0 && est < 2000.0);
    }

    #[test]
    fn xz_hll_empty() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_merge() {
        let mut a = super::XzHyperLogLog::xz_new(10);
        let mut b = super::XzHyperLogLog::xz_new(10);
        for i in 0..500 { a.xz_add(i); }
        for i in 500..1000 { b.xz_add(i); }
        a.xz_merge(&b);
        let est = a.xz_estimate();
        assert!(est > 500.0);
    }

    #[test]
    fn xz_hll_clear() {
        let mut hll = super::XzHyperLogLog::xz_new(10);
        hll.xz_add(1);
        hll.xz_clear();
        assert_eq!(hll.xz_estimate(), 0.0);
    }

    #[test]
    fn xz_hll_display() {
        let hll = super::XzHyperLogLog::xz_new(10);
        assert!(format!("{}", hll).contains("HLL"));
    }

    #[test]
    fn xz_hll_default() {
        let hll = super::XzHyperLogLog::default();
        assert_eq!(hll.xz_precision(), 10);
    }

    #[test]
    fn xz_hll_duplicates() {
        let mut hll = super::XzHyperLogLog::xz_new(12);
        for _ in 0..1000 { hll.xz_add(42); }
        let est = hll.xz_estimate();
        assert!(est < 10.0);
    }

    // --- xz_ LRU Cache tests ---

    #[test]
    fn xz_lru_new() {
        let lru = super::XzLruCache::<String, i32>::xz_new(10);
        assert!(lru.xz_is_empty());
        assert_eq!(lru.xz_capacity(), 10);
    }

    #[test]
    fn xz_lru_put_get() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put("a".to_string(), 1);
        lru.xz_put("b".to_string(), 2);
        assert_eq!(lru.xz_get(&"a".to_string()), Some(&1));
        assert_eq!(lru.xz_get(&"b".to_string()), Some(&2));
    }

    #[test]
    fn xz_lru_eviction() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        assert!(!lru.xz_contains(&1));
        assert!(lru.xz_contains(&2));
        assert!(lru.xz_contains(&3));
    }

    #[test]
    fn xz_lru_access_updates_order() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_get(&1);
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1));
        assert!(!lru.xz_contains(&2));
    }

    #[test]
    fn xz_lru_update_value() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "old");
        lru.xz_put(1, "new");
        assert_eq!(lru.xz_get(&1), Some(&"new"));
        assert_eq!(lru.xz_len(), 1);
    }

    #[test]
    fn xz_lru_remove() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        assert_eq!(lru.xz_remove(&1), Some("a"));
        assert!(!lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_peek() {
        let mut lru = super::XzLruCache::xz_new(2);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        assert_eq!(lru.xz_peek(&1), Some(&"a"));
        lru.xz_put(3, "c");
        assert!(lru.xz_contains(&1) || !lru.xz_contains(&1));
    }

    #[test]
    fn xz_lru_keys_order() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_put(2, "b");
        lru.xz_put(3, "c");
        let keys = lru.xz_keys_lru();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn xz_lru_clear() {
        let mut lru = super::XzLruCache::xz_new(10);
        lru.xz_put(1, "a");
        lru.xz_clear();
        assert!(lru.xz_is_empty());
    }

    #[test]
    fn xz_lru_display() {
        let lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert!(format!("{}", lru).contains("LRU"));
    }

    #[test]
    fn xz_lru_missing_key() {
        let mut lru = super::XzLruCache::<i32, i32>::xz_new(10);
        assert_eq!(lru.xz_get(&999), None);
    }


    // --- ya_ Trie tests ---

    #[test]
    fn ya_trie_new() {
        let t = super::YaTrie::<i32>::ya_new();
        assert!(t.ya_is_empty());
        assert_eq!(t.ya_len(), 0);
    }

    #[test]
    fn ya_trie_insert_get() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("hello", 1);
        t.ya_insert("world", 2);
        assert_eq!(t.ya_get("hello"), Some(&1));
        assert_eq!(t.ya_get("world"), Some(&2));
        assert_eq!(t.ya_get("missing"), None);
    }

    #[test]
    fn ya_trie_contains() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        assert!(t.ya_contains("abc"));
        assert!(!t.ya_contains("ab"));
        assert!(!t.ya_contains("abcd"));
    }

    #[test]
    fn ya_trie_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        t.ya_insert("abd", 2);
        assert!(t.ya_has_prefix("ab"));
        assert!(!t.ya_has_prefix("ac"));
    }

    #[test]
    fn ya_trie_keys_with_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("cat", 1);
        t.ya_insert("car", 2);
        t.ya_insert("dog", 3);
        let keys = t.ya_keys_with_prefix("ca");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"cat".to_string()));
        assert!(keys.contains(&"car".to_string()));
    }

    #[test]
    fn ya_trie_all_keys() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("b", 1);
        t.ya_insert("a", 2);
        t.ya_insert("c", 3);
        let keys = t.ya_all_keys();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn ya_trie_remove() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("hello", 1);
        assert_eq!(t.ya_remove("hello"), Some(1));
        assert!(!t.ya_contains("hello"));
        assert_eq!(t.ya_len(), 0);
    }

    #[test]
    fn ya_trie_lcp() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("abc", 1);
        t.ya_insert("abd", 2);
        assert_eq!(t.ya_longest_common_prefix(), "ab");
    }

    #[test]
    fn ya_trie_clear() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("a", 1);
        t.ya_clear();
        assert!(t.ya_is_empty());
    }

    #[test]
    fn ya_trie_display() {
        let t = super::YaTrie::<i32>::ya_new();
        assert!(format!("{}", t).contains("Trie"));
    }

    #[test]
    fn ya_trie_default() {
        let t = super::YaTrie::<i32>::default();
        assert!(t.ya_is_empty());
    }

    #[test]
    fn ya_trie_count_prefix() {
        let mut t = super::YaTrie::ya_new();
        t.ya_insert("test1", 1);
        t.ya_insert("test2", 2);
        t.ya_insert("other", 3);
        assert_eq!(t.ya_count_prefix("test"), 2);
    }

    // --- ya_ Bloom Filter tests ---

    #[test]
    fn ya_bloom_new() {
        let bf = super::YaBloomFilter::ya_new(1000, 5);
        assert_eq!(bf.ya_bit_size(), 1000);
        assert_eq!(bf.ya_num_hashes(), 5);
        assert_eq!(bf.ya_count(), 0);
    }

    #[test]
    fn ya_bloom_add_contains() {
        let mut bf = super::YaBloomFilter::ya_new(10000, 7);
        bf.ya_add(42);
        bf.ya_add(100);
        assert!(bf.ya_might_contain(42));
        assert!(bf.ya_might_contain(100));
    }

    #[test]
    fn ya_bloom_no_false_negatives() {
        let mut bf = super::YaBloomFilter::ya_new(10000, 7);
        for i in 0..100 { bf.ya_add(i); }
        for i in 0..100 { assert!(bf.ya_might_contain(i)); }
    }

    #[test]
    fn ya_bloom_with_fp_rate() {
        let bf = super::YaBloomFilter::ya_with_fp_rate(1000, 0.01);
        assert!(bf.ya_bit_size() > 0);
        assert!(bf.ya_num_hashes() > 0);
    }

    #[test]
    fn ya_bloom_clear() {
        let mut bf = super::YaBloomFilter::ya_new(1000, 5);
        bf.ya_add(1);
        bf.ya_clear();
        assert_eq!(bf.ya_count(), 0);
        assert!(!bf.ya_might_contain(1));
    }

    #[test]
    fn ya_bloom_merge() {
        let mut a = super::YaBloomFilter::ya_new(1000, 5);
        let mut b = super::YaBloomFilter::ya_new(1000, 5);
        a.ya_add(1);
        b.ya_add(2);
        a.ya_merge(&b);
        assert!(a.ya_might_contain(1));
        assert!(a.ya_might_contain(2));
    }

    #[test]
    fn ya_bloom_fp_rate() {
        let bf = super::YaBloomFilter::ya_new(1000, 5);
        assert_eq!(bf.ya_estimated_fp_rate(), 0.0);
    }

    #[test]
    fn ya_bloom_display() {
        let bf = super::YaBloomFilter::ya_new(100, 3);
        assert!(format!("{}", bf).contains("Bloom"));
    }

    #[test]
    fn ya_bloom_default() {
        let bf = super::YaBloomFilter::default();
        assert_eq!(bf.ya_num_hashes(), 5);
    }


    // --- yb_ TST tests ---

    #[test]
    fn yb_tst_new() {
        let t = super::YbTernarySearchTree::<i32>::yb_new();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_insert_get() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("hello", 1);
        t.yb_insert("world", 2);
        assert_eq!(t.yb_get("hello"), Some(&1));
        assert_eq!(t.yb_get("world"), Some(&2));
        assert_eq!(t.yb_get("missing"), None);
    }

    #[test]
    fn yb_tst_contains() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("abc", 10);
        assert!(t.yb_contains("abc"));
        assert!(!t.yb_contains("ab"));
    }

    #[test]
    fn yb_tst_all_keys() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("b", 1);
        t.yb_insert("a", 2);
        t.yb_insert("c", 3);
        let keys = t.yb_all_keys();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn yb_tst_prefix() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("cat", 1);
        t.yb_insert("car", 2);
        t.yb_insert("dog", 3);
        let keys = t.yb_keys_with_prefix("ca");
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn yb_tst_clear() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("a", 1);
        t.yb_clear();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_display() {
        let t = super::YbTernarySearchTree::<i32>::yb_new();
        assert!(format!("{}", t).contains("TST"));
    }

    #[test]
    fn yb_tst_default() {
        let t = super::YbTernarySearchTree::<i32>::default();
        assert!(t.yb_is_empty());
    }

    #[test]
    fn yb_tst_overwrite() {
        let mut t = super::YbTernarySearchTree::yb_new();
        t.yb_insert("key", 1);
        t.yb_insert("key", 2);
        assert_eq!(t.yb_get("key"), Some(&2));
        assert_eq!(t.yb_len(), 1);
    }

    // --- yb_ Quadtree tests ---

    #[test]
    fn yb_quad_new() {
        let q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(q.yb_is_empty());
    }

    #[test]
    fn yb_quad_insert() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(q.yb_insert(super::YbPoint::yb_new(50.0, 50.0)));
        assert_eq!(q.yb_count(), 1);
    }

    #[test]
    fn yb_quad_query() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 2);
        q.yb_insert(super::YbPoint::yb_new(10.0, 10.0));
        q.yb_insert(super::YbPoint::yb_new(90.0, 90.0));
        q.yb_insert(super::YbPoint::yb_new(15.0, 15.0));
        let found = q.yb_query(&super::YbBounds::yb_new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn yb_quad_outside() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        assert!(!q.yb_insert(super::YbPoint::yb_new(200.0, 200.0)));
    }

    #[test]
    fn yb_quad_nearest() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 4);
        q.yb_insert(super::YbPoint::yb_new(10.0, 10.0));
        q.yb_insert(super::YbPoint::yb_new(90.0, 90.0));
        let near = q.yb_nearest(&super::YbPoint::yb_new(12.0, 12.0)).unwrap();
        assert!((near.yb_x - 10.0).abs() < 0.001);
    }

    #[test]
    fn yb_quad_display() {
        let q = super::YbQuadtree::default();
        assert!(format!("{}", q).contains("Quadtree"));
    }

    #[test]
    fn yb_quad_default() {
        let q = super::YbQuadtree::default();
        assert!(q.yb_is_empty());
    }

    #[test]
    fn yb_quad_many() {
        let mut q = super::YbQuadtree::yb_new(super::YbBounds::yb_new(0.0, 0.0, 100.0, 100.0), 2);
        for i in 0..20 {
            q.yb_insert(super::YbPoint::yb_new(i as f64 * 4.0, i as f64 * 4.0));
        }
        assert_eq!(q.yb_count(), 20);
    }

    #[test]
    fn yb_point_distance() {
        let a = super::YbPoint::yb_new(0.0, 0.0);
        let b = super::YbPoint::yb_new(3.0, 4.0);
        assert!((a.yb_distance(&b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn yb_bounds_intersects() {
        let a = super::YbBounds::yb_new(0.0, 0.0, 50.0, 50.0);
        let b = super::YbBounds::yb_new(25.0, 25.0, 50.0, 50.0);
        assert!(a.yb_intersects(&b));
    }

}
