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

// --- DiffLine helpers ---

impl DiffLine {
    /// Returns `true` if this is a context (unchanged) line.
    pub fn is_context(&self) -> bool {
        self.kind == DiffLineKind::Context
    }

    /// Returns `true` if this is an added line.
    pub fn is_added(&self) -> bool {
        self.kind == DiffLineKind::Added
    }

    /// Returns `true` if this is a removed line.
    pub fn is_removed(&self) -> bool {
        self.kind == DiffLineKind::Removed
    }

    /// Strips the leading `+`, `-`, or ` ` prefix from the content.
    pub fn content_without_prefix(&self) -> &str {
        if self.content.starts_with('+')
            || self.content.starts_with('-')
            || self.content.starts_with(' ')
        {
            &self.content[1..]
        } else {
            &self.content
        }
    }
}

// --- DiffHunk helpers ---

impl DiffHunk {
    /// Returns the number of context (unchanged) lines in this hunk.
    pub fn context_lines(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == DiffLineKind::Context).count()
    }

    /// Net line-count change: added minus removed.
    pub fn net_change(&self) -> isize {
        self.added_lines() as isize - self.removed_lines() as isize
    }

    /// Produces a unified-diff style header for this hunk.
    pub fn header(&self) -> String {
        format!(
            "@@ -{},{} +{},{} @@",
            self.original_start,
            self.original_length,
            self.modified_start,
            self.modified_length,
        )
    }
}

impl fmt::Display for DiffHunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.header())?;
        for line in &self.lines {
            let prefix = match line.kind {
                DiffLineKind::Context => ' ',
                DiffLineKind::Added => '+',
                DiffLineKind::Removed => '-',
            };
            writeln!(f, "{}{}", prefix, line.content)?;
        }
        Ok(())
    }
}

// --- FileDiff helpers ---

impl FileDiff {
    /// Net line-count change across all hunks.
    pub fn net_change(&self) -> isize {
        self.hunks.iter().map(|h| h.net_change()).sum()
    }

    /// Number of hunks in this file diff.
    pub fn hunk_count(&self) -> usize {
        self.hunks.len()
    }

    /// Placeholder: always returns `false` for now.
    pub fn is_binary(&self) -> bool {
        false
    }

    /// Returns `true` if at least one hunk contains changes.
    pub fn has_changes(&self) -> bool {
        self.hunks.iter().any(|h| h.added_lines() > 0 || h.removed_lines() > 0)
    }
}

// --- MultiDiffModel helpers ---

impl MultiDiffModel {
    /// Removes the diff at `index`, returning it if valid.
    pub fn remove_diff(&mut self, index: usize) -> Option<FileDiff> {
        if index < self.diffs.len() {
            Some(self.diffs.remove(index))
        } else {
            None
        }
    }

    /// Finds the first diff whose `display_path()` matches `path`.
    pub fn find_diff_by_path(&self, path: &str) -> Option<&FileDiff> {
        self.diffs.iter().find(|d| d.display_path() == path)
    }

    /// Sorts diffs in-place by their display path.
    pub fn sort_by_path(&mut self) {
        self.diffs.sort_by(|a, b| a.display_path().cmp(b.display_path()));
    }

    /// Returns a human-readable summary of the model.
    pub fn summary(&self) -> String {
        let (a, r, m) = self.stats();
        format!(
            "{} file(s): {} added, {} removed, {} modified | +{} -{} lines",
            self.file_count(),
            a,
            r,
            m,
            self.total_added_lines(),
            self.total_removed_lines(),
        )
    }

    /// Computes aggregate statistics for the whole model.
    pub fn compute_statistics(&self) -> DiffStatistics {
        DiffStatistics {
            total_files: self.file_count(),
            total_added: self.total_added_lines(),
            total_removed: self.total_removed_lines(),
            net_change: self.total_added_lines() as isize - self.total_removed_lines() as isize,
        }
    }

    /// Returns a new model containing only diffs accepted by `filter`.
    pub fn filter(&self, filter: &DiffFilter) -> MultiDiffModel {
        let diffs = self
            .diffs
            .iter()
            .filter(|d| filter.accepts(d))
            .cloned()
            .collect();
        MultiDiffModel { diffs }
    }
}

impl fmt::Display for MultiDiffModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for diff in &self.diffs {
            writeln!(f, "{}", diff)?;
        }
        Ok(())
    }
}

// --- DiffStatistics ---

/// Aggregate statistics for a [`MultiDiffModel`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffStatistics {
    pub total_files: usize,
    pub total_added: usize,
    pub total_removed: usize,
    pub net_change: isize,
}

// --- DiffFilter ---

/// Criteria used to filter file diffs.
#[derive(Debug, Clone)]
pub struct DiffFilter {
    /// If set, only accept diffs of this kind.
    pub kind: Option<DiffKind>,
    /// If set, only accept diffs whose display path contains this substring.
    pub path_contains: Option<String>,
}

impl DiffFilter {
    /// Returns `true` if `diff` passes this filter.
    pub fn accepts(&self, diff: &FileDiff) -> bool {
        if let Some(k) = self.kind {
            if diff.kind != k {
                return false;
            }
        }
        if let Some(ref pat) = self.path_contains {
            if !diff.display_path().contains(pat.as_str()) {
                return false;
            }
        }
        true
    }
}

// --- Hunk utilities ---

/// Applies hunks from a `FileDiff` to `original` text, producing the modified text.
///
/// Lines in the original are 1-indexed to match hunk metadata.
pub fn apply_hunks(original: &str, diff: &FileDiff) -> String {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut pos: usize = 0; // 0-indexed cursor into orig_lines

    for hunk in &diff.hunks {
        let start = (hunk.original_start as usize).saturating_sub(1);
        // Copy lines before this hunk
        while pos < start && pos < orig_lines.len() {
            result.push(orig_lines[pos].to_string());
            pos += 1;
        }
        for line in &hunk.lines {
            match line.kind {
                DiffLineKind::Context => {
                    if pos < orig_lines.len() {
                        result.push(orig_lines[pos].to_string());
                        pos += 1;
                    }
                }
                DiffLineKind::Added => {
                    result.push(line.content_without_prefix().to_string());
                }
                DiffLineKind::Removed => {
                    pos += 1; // skip removed line
                }
            }
        }
    }
    // Copy remaining lines
    while pos < orig_lines.len() {
        result.push(orig_lines[pos].to_string());
        pos += 1;
    }
    result.join("\n")
}

/// Returns a new hunk with added/removed lines swapped.
pub fn invert_hunk(hunk: &DiffHunk) -> DiffHunk {
    let lines = hunk
        .lines
        .iter()
        .map(|l| {
            let new_kind = match l.kind {
                DiffLineKind::Added => DiffLineKind::Removed,
                DiffLineKind::Removed => DiffLineKind::Added,
                DiffLineKind::Context => DiffLineKind::Context,
            };
            DiffLine {
                content: l.content.clone(),
                kind: new_kind,
            }
        })
        .collect();
    DiffHunk {
        original_start: hunk.modified_start,
        original_length: hunk.modified_length,
        modified_start: hunk.original_start,
        modified_length: hunk.original_length,
        lines,
    }
}

/// Returns a new `FileDiff` with every hunk inverted and URIs swapped.
pub fn invert_diff(diff: &FileDiff) -> FileDiff {
    let hunks = diff.hunks.iter().map(|h| invert_hunk(h)).collect();
    FileDiff {
        original_uri: diff.modified_uri.clone(),
        modified_uri: diff.original_uri.clone(),
        kind: diff.kind,
        hunks,
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

    // --- new tests ---

    #[test]
    fn diff_line_helpers() {
        let ctx = DiffLine { content: " hello".into(), kind: DiffLineKind::Context };
        assert!(ctx.is_context());
        assert!(!ctx.is_added());
        assert!(!ctx.is_removed());
        assert_eq!(ctx.content_without_prefix(), "hello");

        let add = DiffLine { content: "+world".into(), kind: DiffLineKind::Added };
        assert!(add.is_added());
        assert_eq!(add.content_without_prefix(), "world");

        let rem = DiffLine { content: "-gone".into(), kind: DiffLineKind::Removed };
        assert!(rem.is_removed());
        assert_eq!(rem.content_without_prefix(), "gone");

        let no_prefix = DiffLine { content: "bare".into(), kind: DiffLineKind::Context };
        assert_eq!(no_prefix.content_without_prefix(), "bare");
    }

    #[test]
    fn hunk_context_lines_and_net_change() {
        let hunk = make_hunk_with_lines();
        assert_eq!(hunk.context_lines(), 1);
        assert_eq!(hunk.net_change(), 1); // +2 -1 = 1
    }

    #[test]
    fn hunk_header_format() {
        let hunk = make_hunk_with_lines();
        assert_eq!(hunk.header(), "@@ -1,3 +1,4 @@");
    }

    #[test]
    fn file_diff_net_change_and_hunk_count() {
        let diff = FileDiff {
            original_uri: Some("a.rs".into()),
            modified_uri: Some("b.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![make_hunk_with_lines(), make_hunk_with_lines()],
        };
        assert_eq!(diff.hunk_count(), 2);
        assert_eq!(diff.net_change(), 2);
        assert!(!diff.is_binary());
        assert!(diff.has_changes());
    }

    #[test]
    fn file_diff_has_no_changes() {
        let diff = FileDiff {
            original_uri: Some("a.rs".into()),
            modified_uri: Some("b.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![DiffHunk {
                original_start: 1,
                original_length: 1,
                modified_start: 1,
                modified_length: 1,
                lines: vec![DiffLine { content: " same".into(), kind: DiffLineKind::Context }],
            }],
        };
        assert!(!diff.has_changes());
    }

    #[test]
    fn model_remove_diff() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Added, 1));
        model.add_diff(make_diff(DiffKind::Removed, 1));
        assert!(model.remove_diff(5).is_none());
        let removed = model.remove_diff(0).unwrap();
        assert_eq!(removed.kind, DiffKind::Added);
        assert_eq!(model.file_count(), 1);
    }

    fn make_named_diff(path: &str, kind: DiffKind) -> FileDiff {
        FileDiff {
            original_uri: None,
            modified_uri: Some(path.into()),
            kind,
            hunks: vec![],
        }
    }

    #[test]
    fn model_find_diff_by_path() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_named_diff("src/main.rs", DiffKind::Modified));
        model.add_diff(make_named_diff("src/lib.rs", DiffKind::Added));
        assert!(model.find_diff_by_path("src/lib.rs").is_some());
        assert!(model.find_diff_by_path("nope.rs").is_none());
    }

    #[test]
    fn model_sort_by_path() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_named_diff("z.rs", DiffKind::Modified));
        model.add_diff(make_named_diff("a.rs", DiffKind::Added));
        model.add_diff(make_named_diff("m.rs", DiffKind::Removed));
        model.sort_by_path();
        let paths: Vec<&str> = model.diffs.iter().map(|d| d.display_path()).collect();
        assert_eq!(paths, vec!["a.rs", "m.rs", "z.rs"]);
    }

    #[test]
    fn model_summary() {
        let mut model = MultiDiffModel::new();
        model.add_diff(FileDiff {
            original_uri: None,
            modified_uri: Some("f.rs".into()),
            kind: DiffKind::Added,
            hunks: vec![make_hunk_with_lines()],
        });
        let s = model.summary();
        assert!(s.contains("1 file(s)"));
        assert!(s.contains("+2"));
        assert!(s.contains("-1"));
    }

    #[test]
    fn compute_statistics() {
        let mut model = MultiDiffModel::new();
        model.add_diff(FileDiff {
            original_uri: Some("a.rs".into()),
            modified_uri: Some("b.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![make_hunk_with_lines()],
        });
        let stats = model.compute_statistics();
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.total_added, 2);
        assert_eq!(stats.total_removed, 1);
        assert_eq!(stats.net_change, 1);
    }

    #[test]
    fn apply_hunks_basic() {
        let original = "line1\nline2\nline3";
        let diff = FileDiff {
            original_uri: Some("a.txt".into()),
            modified_uri: Some("b.txt".into()),
            kind: DiffKind::Modified,
            hunks: vec![DiffHunk {
                original_start: 2,
                original_length: 1,
                modified_start: 2,
                modified_length: 1,
                lines: vec![
                    DiffLine { content: "-line2".into(), kind: DiffLineKind::Removed },
                    DiffLine { content: "+replaced".into(), kind: DiffLineKind::Added },
                ],
            }],
        };
        let result = apply_hunks(original, &diff);
        assert_eq!(result, "line1\nreplaced\nline3");
    }

    #[test]
    fn invert_hunk_swaps() {
        let hunk = make_hunk_with_lines();
        let inv = invert_hunk(&hunk);
        assert_eq!(inv.original_start, hunk.modified_start);
        assert_eq!(inv.modified_start, hunk.original_start);
        assert_eq!(inv.added_lines(), hunk.removed_lines());
        assert_eq!(inv.removed_lines(), hunk.added_lines());
        assert_eq!(inv.context_lines(), hunk.context_lines());
    }

    #[test]
    fn invert_diff_swaps_uris() {
        let diff = FileDiff {
            original_uri: Some("old.rs".into()),
            modified_uri: Some("new.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![make_hunk_with_lines()],
        };
        let inv = invert_diff(&diff);
        assert_eq!(inv.original_uri.as_deref(), Some("new.rs"));
        assert_eq!(inv.modified_uri.as_deref(), Some("old.rs"));
        assert_eq!(inv.hunks[0].added_lines(), 1);
        assert_eq!(inv.hunks[0].removed_lines(), 2);
    }

    #[test]
    fn diff_filter_by_kind() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_named_diff("a.rs", DiffKind::Added));
        model.add_diff(make_named_diff("b.rs", DiffKind::Modified));
        model.add_diff(make_named_diff("c.rs", DiffKind::Added));
        let filter = DiffFilter { kind: Some(DiffKind::Added), path_contains: None };
        let filtered = model.filter(&filter);
        assert_eq!(filtered.file_count(), 2);
    }

    #[test]
    fn diff_filter_by_path() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_named_diff("src/main.rs", DiffKind::Modified));
        model.add_diff(make_named_diff("tests/test.rs", DiffKind::Modified));
        let filter = DiffFilter { kind: None, path_contains: Some("src/".into()) };
        let filtered = model.filter(&filter);
        assert_eq!(filtered.file_count(), 1);
        assert_eq!(filtered.diffs[0].display_path(), "src/main.rs");
    }

    #[test]
    fn display_diff_hunk() {
        let hunk = DiffHunk {
            original_start: 10,
            original_length: 2,
            modified_start: 10,
            modified_length: 3,
            lines: vec![
                DiffLine { content: "ctx".into(), kind: DiffLineKind::Context },
                DiffLine { content: "added".into(), kind: DiffLineKind::Added },
            ],
        };
        let s = format!("{}", hunk);
        assert!(s.starts_with("@@ -10,2 +10,3 @@"));
        assert!(s.contains("+added"));
        assert!(s.contains(" ctx"));
    }

    #[test]
    fn display_multi_diff_model() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_named_diff("a.rs", DiffKind::Added));
        model.add_diff(make_named_diff("b.rs", DiffKind::Modified));
        let s = format!("{}", model);
        assert!(s.contains("a.rs"));
        assert!(s.contains("b.rs"));
    }
}
