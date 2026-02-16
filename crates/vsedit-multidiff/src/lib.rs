//! Multi-file diff model and types.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Added,
    Removed,
    Modified,
    Renamed,
}

impl fmt::Display for DiffKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffKind::Added => write!(f, "Added"),
            DiffKind::Removed => write!(f, "Removed"),
            DiffKind::Modified => write!(f, "Modified"),
            DiffKind::Renamed => write!(f, "Renamed"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

impl fmt::Display for DiffLineKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffLineKind::Context => write!(f, "Context"),
            DiffLineKind::Added => write!(f, "Added"),
            DiffLineKind::Removed => write!(f, "Removed"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub content: String,
    pub kind: DiffLineKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub original_start: u32,
    pub original_length: u32,
    pub modified_start: u32,
    pub modified_length: u32,
    pub lines: Vec<DiffLine>,
}

impl DiffHunk {
    pub fn added_lines(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == DiffLineKind::Added).count()
    }

    pub fn removed_lines(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == DiffLineKind::Removed).count()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub original_uri: Option<String>,
    pub modified_uri: Option<String>,
    pub kind: DiffKind,
    pub hunks: Vec<DiffHunk>,
}

impl FileDiff {
    pub fn total_added(&self) -> usize {
        self.hunks.iter().map(|h| h.added_lines()).sum()
    }

    pub fn total_removed(&self) -> usize {
        self.hunks.iter().map(|h| h.removed_lines()).sum()
    }

    pub fn display_path(&self) -> &str {
        self.modified_uri
            .as_deref()
            .or(self.original_uri.as_deref())
            .unwrap_or("unknown")
    }
}

impl fmt::Display for FileDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} (+{}, -{})", self.kind, self.display_path(), self.total_added(), self.total_removed())
    }
}

pub struct MultiDiffModel {
    pub diffs: Vec<FileDiff>,
}

impl MultiDiffModel {
    pub fn new() -> Self {
        Self { diffs: Vec::new() }
    }

    pub fn add_diff(&mut self, diff: FileDiff) {
        self.diffs.push(diff);
    }

    pub fn file_count(&self) -> usize {
        self.diffs.len()
    }

    pub fn total_hunks(&self) -> usize {
        self.diffs.iter().map(|d| d.hunks.len()).sum()
    }

    /// Returns (added, removed, modified) file counts.
    pub fn stats(&self) -> (usize, usize, usize) {
        let added = self.diffs.iter().filter(|d| d.kind == DiffKind::Added).count();
        let removed = self.diffs.iter().filter(|d| d.kind == DiffKind::Removed).count();
        let modified = self.diffs.iter().filter(|d| d.kind == DiffKind::Modified).count();
        (added, removed, modified)
    }

    pub fn get_diff(&self, index: usize) -> Option<&FileDiff> {
        self.diffs.get(index)
    }

    pub fn total_added_lines(&self) -> usize {
        self.diffs.iter().map(|d| d.total_added()).sum()
    }

    pub fn total_removed_lines(&self) -> usize {
        self.diffs.iter().map(|d| d.total_removed()).sum()
    }

    pub fn get_diffs_by_kind(&self, kind: DiffKind) -> Vec<&FileDiff> {
        self.diffs.iter().filter(|d| d.kind == kind).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.diffs.is_empty()
    }
}

impl Default for MultiDiffModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diff(kind: DiffKind, hunk_count: usize) -> FileDiff {
        let hunks = (0..hunk_count)
            .map(|_| DiffHunk {
                original_start: 1,
                original_length: 5,
                modified_start: 1,
                modified_length: 5,
                lines: vec![],
            })
            .collect();
        FileDiff {
            original_uri: Some("a.txt".into()),
            modified_uri: Some("b.txt".into()),
            kind,
            hunks,
        }
    }

    #[test]
    fn file_count_and_hunks() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Added, 2));
        model.add_diff(make_diff(DiffKind::Modified, 3));
        assert_eq!(model.file_count(), 2);
        assert_eq!(model.total_hunks(), 5);
    }

    #[test]
    fn stats() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Added, 1));
        model.add_diff(make_diff(DiffKind::Added, 1));
        model.add_diff(make_diff(DiffKind::Removed, 1));
        model.add_diff(make_diff(DiffKind::Modified, 1));
        assert_eq!(model.stats(), (2, 1, 1));
    }

    #[test]
    fn empty_model() {
        let model = MultiDiffModel::new();
        assert_eq!(model.file_count(), 0);
        assert_eq!(model.total_hunks(), 0);
        assert_eq!(model.stats(), (0, 0, 0));
        assert!(model.is_empty());
    }

    fn make_hunk_with_lines() -> DiffHunk {
        DiffHunk {
            original_start: 1,
            original_length: 3,
            modified_start: 1,
            modified_length: 4,
            lines: vec![
                DiffLine { content: " ctx".into(), kind: DiffLineKind::Context },
                DiffLine { content: "+new".into(), kind: DiffLineKind::Added },
                DiffLine { content: "+new2".into(), kind: DiffLineKind::Added },
                DiffLine { content: "-old".into(), kind: DiffLineKind::Removed },
            ],
        }
    }

    #[test]
    fn hunk_added_removed_counts() {
        let hunk = make_hunk_with_lines();
        assert_eq!(hunk.added_lines(), 2);
        assert_eq!(hunk.removed_lines(), 1);
        assert!(!hunk.is_empty());
    }

    #[test]
    fn hunk_is_empty() {
        let hunk = DiffHunk {
            original_start: 0,
            original_length: 0,
            modified_start: 0,
            modified_length: 0,
            lines: vec![],
        };
        assert!(hunk.is_empty());
        assert_eq!(hunk.added_lines(), 0);
        assert_eq!(hunk.removed_lines(), 0);
    }

    #[test]
    fn file_diff_totals() {
        let diff = FileDiff {
            original_uri: Some("a.txt".into()),
            modified_uri: Some("b.txt".into()),
            kind: DiffKind::Modified,
            hunks: vec![make_hunk_with_lines(), make_hunk_with_lines()],
        };
        assert_eq!(diff.total_added(), 4);
        assert_eq!(diff.total_removed(), 2);
    }

    #[test]
    fn file_diff_display_path() {
        let both = FileDiff {
            original_uri: Some("orig.rs".into()),
            modified_uri: Some("mod.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![],
        };
        assert_eq!(both.display_path(), "mod.rs");

        let only_orig = FileDiff {
            original_uri: Some("orig.rs".into()),
            modified_uri: None,
            kind: DiffKind::Removed,
            hunks: vec![],
        };
        assert_eq!(only_orig.display_path(), "orig.rs");

        let neither = FileDiff {
            original_uri: None,
            modified_uri: None,
            kind: DiffKind::Added,
            hunks: vec![],
        };
        assert_eq!(neither.display_path(), "unknown");
    }

    #[test]
    fn model_get_diff() {
        let mut model = MultiDiffModel::new();
        assert!(model.get_diff(0).is_none());
        model.add_diff(make_diff(DiffKind::Added, 1));
        assert!(model.get_diff(0).is_some());
        assert!(model.get_diff(1).is_none());
    }

    #[test]
    fn model_total_lines() {
        let mut model = MultiDiffModel::new();
        let diff = FileDiff {
            original_uri: Some("a.txt".into()),
            modified_uri: Some("b.txt".into()),
            kind: DiffKind::Modified,
            hunks: vec![make_hunk_with_lines()],
        };
        model.add_diff(diff);
        assert_eq!(model.total_added_lines(), 2);
        assert_eq!(model.total_removed_lines(), 1);
    }

    #[test]
    fn model_get_diffs_by_kind() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Added, 1));
        model.add_diff(make_diff(DiffKind::Modified, 1));
        model.add_diff(make_diff(DiffKind::Added, 2));
        let added = model.get_diffs_by_kind(DiffKind::Added);
        assert_eq!(added.len(), 2);
        let renamed = model.get_diffs_by_kind(DiffKind::Renamed);
        assert!(renamed.is_empty());
    }

    #[test]
    fn display_diff_kind() {
        assert_eq!(format!("{}", DiffKind::Added), "Added");
        assert_eq!(format!("{}", DiffKind::Removed), "Removed");
        assert_eq!(format!("{}", DiffKind::Modified), "Modified");
        assert_eq!(format!("{}", DiffKind::Renamed), "Renamed");
    }

    #[test]
    fn display_diff_line_kind() {
        assert_eq!(format!("{}", DiffLineKind::Context), "Context");
        assert_eq!(format!("{}", DiffLineKind::Added), "Added");
        assert_eq!(format!("{}", DiffLineKind::Removed), "Removed");
    }

    #[test]
    fn display_file_diff() {
        let diff = FileDiff {
            original_uri: None,
            modified_uri: Some("new_file.rs".into()),
            kind: DiffKind::Added,
            hunks: vec![make_hunk_with_lines()],
        };
        assert_eq!(format!("{}", diff), "Added: new_file.rs (+2, -1)");
    }
}
