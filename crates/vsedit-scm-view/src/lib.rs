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

}
