//! Source control view — SCM sidebar equivalent to VS Code's Git panel.

use std::fmt;
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
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
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
}
