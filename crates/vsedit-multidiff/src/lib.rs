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

// ── Diff navigator ──

/// Navigates between hunks across files in a [`MultiDiffModel`], tracking
/// the current position (file index and hunk index within that file).
pub struct DiffNavigator<'a> {
    model: &'a MultiDiffModel,
    file_index: usize,
    hunk_index: usize,
}

impl<'a> DiffNavigator<'a> {
    pub fn new(model: &'a MultiDiffModel) -> Self {
        Self {
            model,
            file_index: 0,
            hunk_index: 0,
        }
    }

    /// Move to the next hunk, advancing to the next file if needed.
    /// Returns `true` if the position changed.
    pub fn next_hunk(&mut self) -> bool {
        if self.model.diffs.is_empty() {
            return false;
        }
        let file = &self.model.diffs[self.file_index];
        if self.hunk_index + 1 < file.hunks.len() {
            self.hunk_index += 1;
            return true;
        }
        // Try next file with hunks
        for fi in (self.file_index + 1)..self.model.diffs.len() {
            if !self.model.diffs[fi].hunks.is_empty() {
                self.file_index = fi;
                self.hunk_index = 0;
                return true;
            }
        }
        false
    }

    /// Move to the previous hunk, going to the prior file if needed.
    /// Returns `true` if the position changed.
    pub fn prev_hunk(&mut self) -> bool {
        if self.model.diffs.is_empty() {
            return false;
        }
        if self.hunk_index > 0 {
            self.hunk_index -= 1;
            return true;
        }
        for fi in (0..self.file_index).rev() {
            if !self.model.diffs[fi].hunks.is_empty() {
                self.file_index = fi;
                self.hunk_index = self.model.diffs[fi].hunks.len() - 1;
                return true;
            }
        }
        false
    }

    /// Return the current hunk, if any.
    pub fn current_hunk(&self) -> Option<&'a DiffHunk> {
        self.model
            .diffs
            .get(self.file_index)
            .and_then(|f| f.hunks.get(self.hunk_index))
    }

    /// Jump to the first hunk of the file at `file_index`.
    /// Returns `true` if the jump succeeded.
    pub fn jump_to_file(&mut self, file_index: usize) -> bool {
        if file_index < self.model.diffs.len() {
            self.file_index = file_index;
            self.hunk_index = 0;
            true
        } else {
            false
        }
    }

    /// Current file index.
    pub fn current_file_index(&self) -> usize {
        self.file_index
    }

    /// Current hunk index within the current file.
    pub fn current_hunk_index(&self) -> usize {
        self.hunk_index
    }
}

// ---------------------------------------------------------------------------
// Diff statistics aggregation
// ---------------------------------------------------------------------------

/// Per-file diff statistics with additions, deletions, and modifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiffStats {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
    pub modifications: usize,
}

impl FileDiffStats {
    /// Compute stats for a single `FileDiff`.
    pub fn from_diff(diff: &FileDiff) -> Self {
        let mut additions = 0usize;
        let mut deletions = 0usize;
        for hunk in &diff.hunks {
            additions += hunk.added_lines();
            deletions += hunk.removed_lines();
        }
        let modifications = additions.min(deletions);
        Self {
            path: diff.display_path().to_string(),
            additions: additions.saturating_sub(modifications),
            deletions: deletions.saturating_sub(modifications),
            modifications,
        }
    }

    /// Total number of changed lines (additions + deletions + modifications).
    pub fn total_changes(&self) -> usize {
        self.additions + self.deletions + self.modifications
    }
}

/// Compute per-file stats for an entire [`MultiDiffModel`].
pub fn compute_per_file_stats(model: &MultiDiffModel) -> Vec<FileDiffStats> {
    model.diffs.iter().map(|d| FileDiffStats::from_diff(d)).collect()
}

// ---------------------------------------------------------------------------
// Hunk range intersection
// ---------------------------------------------------------------------------

/// A line range `[start, end)` (half-open, 1-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

impl LineRange {
    /// Create a new line range.
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Length of the range.
    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Whether the range is empty.
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// Whether this range overlaps with another.
    pub fn overlaps(&self, other: &LineRange) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Compute the intersection with another range, or `None` if disjoint.
    pub fn intersect(&self, other: &LineRange) -> Option<LineRange> {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        if start < end {
            Some(LineRange { start, end })
        } else {
            None
        }
    }
}

/// Get the original-side line range of a hunk.
pub fn hunk_original_range(hunk: &DiffHunk) -> LineRange {
    LineRange::new(hunk.original_start, hunk.original_start + hunk.original_length)
}

/// Get the modified-side line range of a hunk.
pub fn hunk_modified_range(hunk: &DiffHunk) -> LineRange {
    LineRange::new(hunk.modified_start, hunk.modified_start + hunk.modified_length)
}

// ---------------------------------------------------------------------------
// Hunk merging
// ---------------------------------------------------------------------------

/// Merge adjacent hunks that are within `max_gap` lines of each other.
/// Hunks must be sorted by `original_start`.
pub fn merge_adjacent_hunks(hunks: &[DiffHunk], max_gap: u32) -> Vec<DiffHunk> {
    if hunks.is_empty() {
        return Vec::new();
    }
    let mut result: Vec<DiffHunk> = vec![hunks[0].clone()];
    for hunk in &hunks[1..] {
        let last = result.last_mut().unwrap();
        let last_end = last.original_start + last.original_length;
        if hunk.original_start <= last_end + max_gap {
            // Merge: extend the last hunk to cover both
            let new_end = (hunk.original_start + hunk.original_length).max(last_end);
            last.original_length = new_end - last.original_start;
            let mod_end = (hunk.modified_start + hunk.modified_length)
                .max(last.modified_start + last.modified_length);
            last.modified_length = mod_end - last.modified_start;
            last.lines.extend(hunk.lines.iter().cloned());
        } else {
            result.push(hunk.clone());
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Diff navigation helpers
// ---------------------------------------------------------------------------

impl<'a> DiffNavigator<'a> {
    /// Total number of hunks across all files.
    pub fn total_hunks(&self) -> usize {
        self.model.total_hunks()
    }

    /// Returns a flat index of the current hunk among all hunks across all files.
    pub fn flat_hunk_index(&self) -> usize {
        let mut idx = 0;
        for fi in 0..self.file_index {
            idx += self.model.diffs[fi].hunks.len();
        }
        idx + self.hunk_index
    }
}

// ---------------------------------------------------------------------------
// Diff change summary per-file
// ---------------------------------------------------------------------------

/// A one-line summary of changes for a single file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffChangeSummary {
    pub path: String,
    pub kind: DiffKind,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: usize,
}

impl DiffChangeSummary {
    pub fn from_file_diff(diff: &FileDiff) -> Self {
        Self {
            path: diff.display_path().to_string(),
            kind: diff.kind,
            additions: diff.total_added(),
            deletions: diff.total_removed(),
            hunks: diff.hunk_count(),
        }
    }

    /// Net line change (additions - deletions).
    pub fn net(&self) -> isize {
        self.additions as isize - self.deletions as isize
    }
}

impl fmt::Display for DiffChangeSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} (+{} -{} ~{} hunks)",
            self.kind, self.path, self.additions, self.deletions, self.hunks,
        )
    }
}

/// Generate change summaries for all files in a model.
pub fn change_summaries(model: &MultiDiffModel) -> Vec<DiffChangeSummary> {
    model.diffs.iter().map(DiffChangeSummary::from_file_diff).collect()
}

// ---------------------------------------------------------------------------
// Unified diff output
// ---------------------------------------------------------------------------

/// Produce a unified diff string for a single FileDiff.
pub fn unified_diff(diff: &FileDiff) -> String {
    let mut out = String::new();
    let orig = diff.original_uri.as_deref().unwrap_or("/dev/null");
    let modified = diff.modified_uri.as_deref().unwrap_or("/dev/null");
    out.push_str(&format!("--- {}\n", orig));
    out.push_str(&format!("+++ {}\n", modified));
    for hunk in &diff.hunks {
        out.push_str(&format!("{}\n", hunk.header()));
        for line in &hunk.lines {
            let prefix = match line.kind {
                DiffLineKind::Context => ' ',
                DiffLineKind::Added => '+',
                DiffLineKind::Removed => '-',
            };
            out.push_str(&format!("{}{}\n", prefix, line.content));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// MultiDiffModel — path-based operations
// ---------------------------------------------------------------------------

impl MultiDiffModel {
    /// Return all file paths affected by the diff (display paths).
    pub fn affected_paths(&self) -> Vec<&str> {
        self.diffs.iter().map(|d| d.display_path()).collect()
    }

    /// Return the total net line change across all files.
    pub fn net_change(&self) -> isize {
        self.total_added_lines() as isize - self.total_removed_lines() as isize
    }

    /// Return diffs that match a file extension (e.g. "rs", "ts").
    pub fn diffs_by_extension(&self, ext: &str) -> Vec<&FileDiff> {
        let suffix = format!(".{}", ext);
        self.diffs.iter().filter(|d| d.display_path().ends_with(&suffix)).collect()
    }

    /// Return the largest diff (most total changed lines).
    pub fn largest_diff(&self) -> Option<&FileDiff> {
        self.diffs.iter().max_by_key(|d| d.total_added() + d.total_removed())
    }
}

// ---------------------------------------------------------------------------
// MultiDiffStatsSummary – aggregate stats across all files
// ---------------------------------------------------------------------------

/// Aggregated statistics across all files in a multi-diff.
#[derive(Debug, Clone)]
pub struct MultiDiffStatsSummary {
    pub total_files: usize,
    pub total_hunks: usize,
    pub total_additions: usize,
    pub total_deletions: usize,
    pub files_added: usize,
    pub files_removed: usize,
    pub files_modified: usize,
    pub files_renamed: usize,
}

impl MultiDiffStatsSummary {
    /// Compute summary from a multi-diff model.
    pub fn from_model(model: &MultiDiffModel) -> Self {
        let mut s = Self {
            total_files: model.file_count(),
            total_hunks: model.total_hunks(),
            total_additions: model.total_added_lines(),
            total_deletions: model.total_removed_lines(),
            files_added: 0,
            files_removed: 0,
            files_modified: 0,
            files_renamed: 0,
        };
        for d in &model.diffs {
            match d.kind {
                DiffKind::Added => s.files_added += 1,
                DiffKind::Removed => s.files_removed += 1,
                DiffKind::Modified => s.files_modified += 1,
                DiffKind::Renamed => s.files_renamed += 1,
            }
        }
        s
    }

    /// Net line change (additions - deletions).
    pub fn net_change(&self) -> isize {
        self.total_additions as isize - self.total_deletions as isize
    }

    /// Format a short summary string.
    pub fn short_summary(&self) -> String {
        format!(
            "{} files (+{} -{}) {} hunks",
            self.total_files, self.total_additions, self.total_deletions, self.total_hunks
        )
    }
}

// ---------------------------------------------------------------------------
// MultiDiffCollapse – toggling file visibility
// ---------------------------------------------------------------------------

/// Manages which files are collapsed (hidden) in the multi-diff view.
pub struct MultiDiffCollapse {
    collapsed: Vec<bool>,
}

impl MultiDiffCollapse {
    /// Create with all files expanded.
    pub fn new(file_count: usize) -> Self {
        Self { collapsed: vec![false; file_count] }
    }

    /// Toggle the collapsed state of a file. Returns the new state.
    pub fn toggle(&mut self, index: usize) -> Option<bool> {
        if index < self.collapsed.len() {
            self.collapsed[index] = !self.collapsed[index];
            Some(self.collapsed[index])
        } else {
            None
        }
    }

    /// Check if a file is collapsed.
    pub fn is_collapsed(&self, index: usize) -> bool {
        self.collapsed.get(index).copied().unwrap_or(false)
    }

    /// Collapse all files.
    pub fn collapse_all(&mut self) {
        for c in &mut self.collapsed {
            *c = true;
        }
    }

    /// Expand all files.
    pub fn expand_all(&mut self) {
        for c in &mut self.collapsed {
            *c = false;
        }
    }

    /// Number of expanded files.
    pub fn expanded_count(&self) -> usize {
        self.collapsed.iter().filter(|&&c| !c).count()
    }

    /// Number of collapsed files.
    pub fn collapsed_count(&self) -> usize {
        self.collapsed.iter().filter(|&&c| c).count()
    }
}

// ---------------------------------------------------------------------------
// MultiDiffHunkAction – accept/reject per hunk
// ---------------------------------------------------------------------------

/// Action taken on a hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkAction {
    /// Hunk has not been acted upon.
    Pending,
    /// Hunk was accepted (changes will be applied).
    Accepted,
    /// Hunk was rejected (changes will be discarded).
    Rejected,
}

/// Tracks accept/reject decisions per hunk across all files.
pub struct MultiDiffHunkActions {
    /// Indexed as actions[file_index][hunk_index].
    actions: Vec<Vec<HunkAction>>,
}

impl MultiDiffHunkActions {
    /// Create from a model with all hunks pending.
    pub fn from_model(model: &MultiDiffModel) -> Self {
        let actions = model.diffs.iter()
            .map(|d| vec![HunkAction::Pending; d.hunks.len()])
            .collect();
        Self { actions }
    }

    /// Set the action for a specific hunk.
    pub fn set_action(&mut self, file_index: usize, hunk_index: usize, action: HunkAction) -> bool {
        if let Some(hunks) = self.actions.get_mut(file_index) {
            if let Some(h) = hunks.get_mut(hunk_index) {
                *h = action;
                return true;
            }
        }
        false
    }

    /// Get the action for a specific hunk.
    pub fn get_action(&self, file_index: usize, hunk_index: usize) -> HunkAction {
        self.actions.get(file_index)
            .and_then(|hunks| hunks.get(hunk_index))
            .copied()
            .unwrap_or(HunkAction::Pending)
    }

    /// Accept all hunks in a file.
    pub fn accept_all_in_file(&mut self, file_index: usize) {
        if let Some(hunks) = self.actions.get_mut(file_index) {
            for h in hunks {
                *h = HunkAction::Accepted;
            }
        }
    }

    /// Reject all hunks in a file.
    pub fn reject_all_in_file(&mut self, file_index: usize) {
        if let Some(hunks) = self.actions.get_mut(file_index) {
            for h in hunks {
                *h = HunkAction::Rejected;
            }
        }
    }

    /// Count of pending hunks across all files.
    pub fn pending_count(&self) -> usize {
        self.actions.iter().flat_map(|v| v.iter()).filter(|&&a| a == HunkAction::Pending).count()
    }

    /// Count of accepted hunks.
    pub fn accepted_count(&self) -> usize {
        self.actions.iter().flat_map(|v| v.iter()).filter(|&&a| a == HunkAction::Accepted).count()
    }

    /// Whether all hunks have been acted upon.
    pub fn is_all_resolved(&self) -> bool {
        self.pending_count() == 0
    }
}

// ---------------------------------------------------------------------------
// MultiDiffSearch – search across all diffs
// ---------------------------------------------------------------------------

/// A single search match across multiple diff files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiDiffSearchResult {
    pub file_path: String,
    pub line_num: usize,
    pub line_text: String,
    pub match_start: usize,
    pub match_end: usize,
}

impl fmt::Display for MultiDiffSearchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {} ({}..{})",
            self.file_path, self.line_num, self.line_text, self.match_start, self.match_end,
        )
    }
}

/// Searches across content from multiple diff files.
#[derive(Debug, Clone, Default)]
pub struct MultiDiffSearch {
    entries: Vec<(String, String)>,
}

impl MultiDiffSearch {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add_diff(&mut self, file_path: &str, content: &str) {
        self.entries.push((file_path.to_string(), content.to_string()));
    }

    pub fn search(&self, query: &str) -> Vec<MultiDiffSearchResult> {
        let mut results = Vec::new();
        if query.is_empty() {
            return results;
        }
        for (path, content) in &self.entries {
            for (idx, line) in content.lines().enumerate() {
                let mut start = 0;
                while let Some(pos) = line[start..].find(query) {
                    let abs = start + pos;
                    results.push(MultiDiffSearchResult {
                        file_path: path.clone(),
                        line_num: idx + 1,
                        line_text: line.to_string(),
                        match_start: abs,
                        match_end: abs + query.len(),
                    });
                    start = abs + query.len();
                }
            }
        }
        results
    }

    pub fn search_count(&self, query: &str) -> usize {
        self.search(query).len()
    }

    pub fn file_count(&self) -> usize {
        self.entries.len()
    }
}

impl fmt::Display for MultiDiffSearch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MultiDiffSearch({} files)", self.entries.len())
    }
}

// ---------------------------------------------------------------------------
// MultiDiffExport – export diffs to unified patch format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ExportEntry {
    file_path: String,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
}

/// Exports diffs to unified patch format.
#[derive(Debug, Clone, Default)]
pub struct MultiDiffExport {
    entries: Vec<ExportEntry>,
}

impl MultiDiffExport {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add_file_diff(&mut self, file_path: &str, old_lines: &[&str], new_lines: &[&str]) {
        self.entries.push(ExportEntry {
            file_path: file_path.to_string(),
            old_lines: old_lines.iter().map(|s| s.to_string()).collect(),
            new_lines: new_lines.iter().map(|s| s.to_string()).collect(),
        });
    }

    pub fn to_unified_diff(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            out.push_str(&format!("--- a/{}\n", entry.file_path));
            out.push_str(&format!("+++ b/{}\n", entry.file_path));
            let old_len = entry.old_lines.len();
            let new_len = entry.new_lines.len();
            out.push_str(&format!("@@ -1,{old_len} +1,{new_len} @@\n"));

            let max = old_len.max(new_len);
            let mut oi = 0;
            let mut ni = 0;
            while oi < old_len || ni < new_len {
                if oi < old_len && ni < new_len && entry.old_lines[oi] == entry.new_lines[ni] {
                    out.push_str(&format!(" {}\n", entry.old_lines[oi]));
                    oi += 1;
                    ni += 1;
                } else {
                    // emit removals first, then additions
                    let mut removed = 0;
                    let save_oi = oi;
                    while oi < old_len
                        && (ni >= new_len || entry.old_lines[oi] != entry.new_lines[ni])
                    {
                        out.push_str(&format!("-{}\n", entry.old_lines[oi]));
                        oi += 1;
                        removed += 1;
                        if removed >= (max - save_oi) {
                            break;
                        }
                    }
                    while ni < new_len
                        && (oi >= old_len || entry.new_lines[ni] != entry.old_lines[oi])
                    {
                        out.push_str(&format!("+{}\n", entry.new_lines[ni]));
                        ni += 1;
                    }
                }
            }
        }
        out
    }

    pub fn file_count(&self) -> usize {
        self.entries.len()
    }
}

impl fmt::Display for MultiDiffExport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MultiDiffExport({} files)", self.entries.len())
    }
}

// ---------------------------------------------------------------------------
// MultiDiffMerge – merge adjacent hunks
// ---------------------------------------------------------------------------

/// A hunk resulting from merging adjacent hunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedHunk {
    pub file: String,
    pub start: usize,
    pub old_count: usize,
    pub new_count: usize,
    pub content: String,
}

impl fmt::Display for MergedHunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{} -{}/+{}",
            self.file, self.start, self.old_count, self.new_count,
        )
    }
}

#[derive(Debug, Clone)]
struct RawHunk {
    file: String,
    start: usize,
    old_count: usize,
    new_count: usize,
    content: String,
}

/// Combines multiple hunks, merging those within a gap threshold.
#[derive(Debug, Clone, Default)]
pub struct MultiDiffMerge {
    hunks: Vec<RawHunk>,
}

impl MultiDiffMerge {
    pub fn new() -> Self {
        Self { hunks: Vec::new() }
    }

    pub fn add_hunk(&mut self, file: &str, start: usize, old_count: usize, new_count: usize, content: &str) {
        self.hunks.push(RawHunk {
            file: file.to_string(),
            start,
            old_count,
            new_count,
            content: content.to_string(),
        });
    }

    /// Merge hunks for the same file that are within `max_gap` lines of each other.
    pub fn merge_adjacent(&self, max_gap: usize) -> Vec<MergedHunk> {
        // Group by file, preserving insertion order within each file.
        let mut by_file: Vec<(String, Vec<&RawHunk>)> = Vec::new();
        for h in &self.hunks {
            if let Some(entry) = by_file.iter_mut().find(|(f, _)| *f == h.file) {
                entry.1.push(h);
            } else {
                by_file.push((h.file.clone(), vec![h]));
            }
        }

        let mut merged = Vec::new();
        for (file, mut group) in by_file {
            group.sort_by_key(|h| h.start);

            let mut cur_start = group[0].start;
            let mut cur_old = group[0].old_count;
            let mut cur_new = group[0].new_count;
            let mut cur_content = group[0].content.clone();

            for h in group.iter().skip(1) {
                let cur_end = cur_start + cur_old;
                if h.start <= cur_end + max_gap {
                    let gap = h.start.saturating_sub(cur_end);
                    for _ in 0..gap {
                        cur_content.push('\n');
                    }
                    cur_content.push_str(&h.content);
                    let new_end = (cur_start + cur_old).max(h.start + h.old_count);
                    cur_old = new_end - cur_start;
                    let new_new_end = (cur_start + cur_new).max(h.start + h.new_count);
                    cur_new = new_new_end - cur_start;
                } else {
                    merged.push(MergedHunk {
                        file: file.clone(),
                        start: cur_start,
                        old_count: cur_old,
                        new_count: cur_new,
                        content: cur_content,
                    });
                    cur_start = h.start;
                    cur_old = h.old_count;
                    cur_new = h.new_count;
                    cur_content = h.content.clone();
                }
            }
            merged.push(MergedHunk {
                file: file.clone(),
                start: cur_start,
                old_count: cur_old,
                new_count: cur_new,
                content: cur_content,
            });
        }
        merged
    }

    pub fn hunk_count(&self) -> usize {
        self.hunks.len()
    }
}

impl fmt::Display for MultiDiffMerge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MultiDiffMerge({} hunks)", self.hunks.len())
    }
}

// ---------------------------------------------------------------------------
// MultiDiffFilter – filter diffs by change type
// ---------------------------------------------------------------------------

/// The kind of change for filtering purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffChangeKind {
    Added,
    Removed,
    Modified,
    Renamed,
}

impl fmt::Display for DiffChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffChangeKind::Added => write!(f, "Added"),
            DiffChangeKind::Removed => write!(f, "Removed"),
            DiffChangeKind::Modified => write!(f, "Modified"),
            DiffChangeKind::Renamed => write!(f, "Renamed"),
        }
    }
}

/// Filters diff files by their change type.
#[derive(Debug, Clone, Default)]
pub struct MultiDiffFilter {
    changes: Vec<(String, DiffChangeKind)>,
}

impl MultiDiffFilter {
    pub fn new() -> Self {
        Self { changes: Vec::new() }
    }

    pub fn add_change(&mut self, file: &str, kind: DiffChangeKind) {
        self.changes.push((file.to_string(), kind));
    }

    pub fn files_with_kind(&self, kind: DiffChangeKind) -> Vec<&str> {
        self.changes
            .iter()
            .filter(|(_, k)| *k == kind)
            .map(|(f, _)| f.as_str())
            .collect()
    }

    pub fn filter_by_kind(&self, kind: DiffChangeKind) -> Vec<&str> {
        self.files_with_kind(kind)
    }

    pub fn total_files(&self) -> usize {
        self.changes.len()
    }

    pub fn count_by_kind(&self, kind: DiffChangeKind) -> usize {
        self.changes.iter().filter(|(_, k)| *k == kind).count()
    }
}

impl fmt::Display for MultiDiffFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MultiDiffFilter({} files)", self.changes.len())
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 126
// ---------------------------------------------------------------------------

/// Generic object pool `Xc126Pool<T>`.
pub struct Xc126Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc126Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc126PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc126Pool<T> {
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
    pub fn stats(&self) -> Xc126PoolStats {
        Xc126PoolStats {
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

impl<T> Default for Xc126Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc126Scheduler`.
pub struct Xc126Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc126Scheduler {
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

impl Default for Xc126Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_126 hash for the given byte slice.
pub fn xc_126_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_126 convention.
pub fn xc_126_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_96 deepening: state machine + event bus ---

/// States for the Xd96 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd96State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd96State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd96Transition {
    pub from: Xd96State,
    pub to: Xd96State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd96StateMachine {
    current: Xd96State,
    history: Vec<Xd96Transition>,
    step_counter: usize,
}

impl Xd96StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd96State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd96State {
        self.current
    }

    pub fn history(&self) -> &[Xd96Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd96State) -> Result<Xd96State, String> {
        let allowed = match (self.current, target) {
            (Xd96State::Idle, Xd96State::Running) => true,
            (Xd96State::Running, Xd96State::Paused) => true,
            (Xd96State::Running, Xd96State::Done) => true,
            (Xd96State::Paused, Xd96State::Running) => true,
            (Xd96State::Paused, Xd96State::Done) => true,
            (Xd96State::Done, Xd96State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_96: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd96Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd96SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd96State> {
        let prefix = "Xd96SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd96State::Idle),
            "Running" => Some(Xd96State::Running),
            "Paused" => Some(Xd96State::Paused),
            "Done" => Some(Xd96State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd96State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd96 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd96Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd96Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd96HandlerFn = Box<dyn Fn(&Xd96Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd96EventBus {
    handlers: Vec<(usize, Option<String>, Xd96HandlerFn)>,
    next_id: usize,
    published: Vec<Xd96Event>,
}

impl Xd96EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd96Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd96Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd96Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd96Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 125).
pub struct Xh125SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh125SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 167 as u64,
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

/// A compact bit set supporting boolean operations (variant 125).
pub struct Xh125BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh125BitSet {
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


// --- xj_ Union-Find and B-Tree (crate index 125) ---

/// Disjoint set / union-find for crate 125.
pub struct Xj125UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj125UnionFind {
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

const XJ125_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 125.
pub struct Xj125BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj125BTreeNode<K, V>>>,
    len: usize,
}

struct Xj125BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj125BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj125BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ125_BTREE_ORDER - 1
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
        let mid = XJ125_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj125BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj125BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj125BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj125BTreeNode::xj_new_leaf();
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


// --- xk_125 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk125SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk125SegmentTree {
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
pub struct Xk125DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk125DisjointIntervals {
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


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm125MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm125MatrixSparse {
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
pub struct Xm125Tokenizer {
    text: String,
}

impl Xm125Tokenizer {
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

    // ── DiffNavigator tests ──

    fn make_model_with_hunks() -> MultiDiffModel {
        let mut model = MultiDiffModel::new();
        model.add_diff(FileDiff {
            original_uri: Some("a.rs".into()),
            modified_uri: Some("a.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![make_hunk_with_lines(), make_hunk_with_lines()],
        });
        model.add_diff(FileDiff {
            original_uri: Some("b.rs".into()),
            modified_uri: Some("b.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![make_hunk_with_lines()],
        });
        model
    }

    #[test]
    fn navigator_next_within_file() {
        let model = make_model_with_hunks();
        let mut nav = DiffNavigator::new(&model);
        assert_eq!(nav.current_file_index(), 0);
        assert_eq!(nav.current_hunk_index(), 0);
        assert!(nav.next_hunk());
        assert_eq!(nav.current_file_index(), 0);
        assert_eq!(nav.current_hunk_index(), 1);
    }

    #[test]
    fn navigator_next_crosses_file() {
        let model = make_model_with_hunks();
        let mut nav = DiffNavigator::new(&model);
        nav.next_hunk(); // hunk 1 in file 0
        assert!(nav.next_hunk()); // crosses to file 1, hunk 0
        assert_eq!(nav.current_file_index(), 1);
        assert_eq!(nav.current_hunk_index(), 0);
    }

    #[test]
    fn navigator_next_at_end_returns_false() {
        let model = make_model_with_hunks();
        let mut nav = DiffNavigator::new(&model);
        nav.next_hunk();
        nav.next_hunk();
        assert!(!nav.next_hunk()); // no more hunks
    }

    #[test]
    fn navigator_prev_crosses_file() {
        let model = make_model_with_hunks();
        let mut nav = DiffNavigator::new(&model);
        nav.jump_to_file(1);
        assert!(nav.prev_hunk());
        assert_eq!(nav.current_file_index(), 0);
        assert_eq!(nav.current_hunk_index(), 1); // last hunk of file 0
    }

    #[test]
    fn navigator_current_hunk() {
        let model = make_model_with_hunks();
        let nav = DiffNavigator::new(&model);
        assert!(nav.current_hunk().is_some());
    }

    #[test]
    fn navigator_jump_to_file() {
        let model = make_model_with_hunks();
        let mut nav = DiffNavigator::new(&model);
        assert!(nav.jump_to_file(1));
        assert_eq!(nav.current_file_index(), 1);
        assert!(!nav.jump_to_file(99));
    }

    #[test]
    fn navigator_empty_model() {
        let model = MultiDiffModel::new();
        let mut nav = DiffNavigator::new(&model);
        assert!(!nav.next_hunk());
        assert!(!nav.prev_hunk());
        assert!(nav.current_hunk().is_none());
    }

    #[test]
    fn file_diff_stats_pure_additions() {
        let diff = FileDiff {
            original_uri: None,
            modified_uri: Some("new.txt".into()),
            kind: DiffKind::Added,
            hunks: vec![DiffHunk {
                original_start: 1, original_length: 0,
                modified_start: 1, modified_length: 3,
                lines: vec![
                    DiffLine { content: "+a".into(), kind: DiffLineKind::Added },
                    DiffLine { content: "+b".into(), kind: DiffLineKind::Added },
                    DiffLine { content: "+c".into(), kind: DiffLineKind::Added },
                ],
            }],
        };
        let stats = FileDiffStats::from_diff(&diff);
        assert_eq!(stats.additions, 3);
        assert_eq!(stats.deletions, 0);
        assert_eq!(stats.modifications, 0);
        assert_eq!(stats.total_changes(), 3);
    }

    #[test]
    fn file_diff_stats_mixed_changes() {
        let diff = FileDiff {
            original_uri: Some("a.txt".into()),
            modified_uri: Some("a.txt".into()),
            kind: DiffKind::Modified,
            hunks: vec![DiffHunk {
                original_start: 1, original_length: 2,
                modified_start: 1, modified_length: 3,
                lines: vec![
                    DiffLine { content: "-old".into(), kind: DiffLineKind::Removed },
                    DiffLine { content: "+new1".into(), kind: DiffLineKind::Added },
                    DiffLine { content: "+new2".into(), kind: DiffLineKind::Added },
                ],
            }],
        };
        let stats = FileDiffStats::from_diff(&diff);
        assert_eq!(stats.modifications, 1);
        assert_eq!(stats.additions, 1);
        assert_eq!(stats.deletions, 0);
    }

    #[test]
    fn line_range_overlap_and_intersect() {
        let a = LineRange::new(1, 5);
        let b = LineRange::new(3, 8);
        assert!(a.overlaps(&b));
        let inter = a.intersect(&b).unwrap();
        assert_eq!(inter.start, 3);
        assert_eq!(inter.end, 5);
        assert_eq!(inter.len(), 2);
    }

    #[test]
    fn line_range_no_overlap() {
        let a = LineRange::new(1, 3);
        let b = LineRange::new(5, 8);
        assert!(!a.overlaps(&b));
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn line_range_empty() {
        let r = LineRange::new(5, 5);
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn hunk_range_accessors() {
        let hunk = DiffHunk {
            original_start: 10, original_length: 5,
            modified_start: 12, modified_length: 3,
            lines: vec![],
        };
        let orig = hunk_original_range(&hunk);
        assert_eq!(orig, LineRange::new(10, 15));
        let modif = hunk_modified_range(&hunk);
        assert_eq!(modif, LineRange::new(12, 15));
    }

    #[test]
    fn merge_adjacent_hunks_basic() {
        let hunks = vec![
            DiffHunk {
                original_start: 1, original_length: 3,
                modified_start: 1, modified_length: 3,
                lines: vec![],
            },
            DiffHunk {
                original_start: 5, original_length: 2,
                modified_start: 5, modified_length: 2,
                lines: vec![],
            },
            DiffHunk {
                original_start: 20, original_length: 1,
                modified_start: 20, modified_length: 1,
                lines: vec![],
            },
        ];
        let merged = merge_adjacent_hunks(&hunks, 1);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].original_start, 1);
        assert_eq!(merged[0].original_length, 6);
    }

    #[test]
    fn merge_adjacent_hunks_empty() {
        let merged = merge_adjacent_hunks(&[], 5);
        assert!(merged.is_empty());
    }

    #[test]
    fn navigator_flat_hunk_index() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Modified, 3));
        model.add_diff(make_diff(DiffKind::Modified, 2));
        let mut nav = DiffNavigator::new(&model);
        assert_eq!(nav.flat_hunk_index(), 0);
        nav.next_hunk();
        assert_eq!(nav.flat_hunk_index(), 1);
        nav.next_hunk();
        nav.next_hunk();
        assert_eq!(nav.flat_hunk_index(), 3);
    }

    #[test]
    fn compute_per_file_stats_works() {
        let mut model = MultiDiffModel::new();
        model.add_diff(FileDiff {
            original_uri: Some("a.rs".into()),
            modified_uri: Some("a.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![DiffHunk {
                original_start: 1, original_length: 1,
                modified_start: 1, modified_length: 1,
                lines: vec![
                    DiffLine { content: "-x".into(), kind: DiffLineKind::Removed },
                    DiffLine { content: "+y".into(), kind: DiffLineKind::Added },
                ],
            }],
        });
        let stats = compute_per_file_stats(&model);
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].modifications, 1);
    }

    // -- New tests ----------------------------------------------------------

    #[test]
    fn change_summary_from_file_diff() {
        let diff = FileDiff {
            original_uri: Some("old.rs".into()),
            modified_uri: Some("new.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![DiffHunk {
                original_start: 1, original_length: 2,
                modified_start: 1, modified_length: 3,
                lines: vec![
                    DiffLine { content: "-old".into(), kind: DiffLineKind::Removed },
                    DiffLine { content: "+new1".into(), kind: DiffLineKind::Added },
                    DiffLine { content: "+new2".into(), kind: DiffLineKind::Added },
                ],
            }],
        };
        let summary = DiffChangeSummary::from_file_diff(&diff);
        assert_eq!(summary.path, "new.rs");
        assert_eq!(summary.additions, 2);
        assert_eq!(summary.deletions, 1);
        assert_eq!(summary.net(), 1);
        assert!(format!("{}", summary).contains("Modified"));
    }

    #[test]
    fn change_summaries_for_model() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Added, 1));
        model.add_diff(make_diff(DiffKind::Removed, 2));
        let summaries = change_summaries(&model);
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].kind, DiffKind::Added);
        assert_eq!(summaries[1].kind, DiffKind::Removed);
    }

    #[test]
    fn unified_diff_output() {
        let diff = FileDiff {
            original_uri: Some("a.rs".into()),
            modified_uri: Some("b.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![DiffHunk {
                original_start: 1, original_length: 1,
                modified_start: 1, modified_length: 1,
                lines: vec![
                    DiffLine { content: "hello".into(), kind: DiffLineKind::Removed },
                    DiffLine { content: "world".into(), kind: DiffLineKind::Added },
                ],
            }],
        };
        let output = unified_diff(&diff);
        assert!(output.starts_with("--- a.rs\n+++ b.rs\n"));
        assert!(output.contains("-hello"));
        assert!(output.contains("+world"));
    }

    #[test]
    fn affected_paths_and_net_change() {
        let mut model = MultiDiffModel::new();
        model.add_diff(FileDiff {
            original_uri: None,
            modified_uri: Some("src/main.rs".into()),
            kind: DiffKind::Added,
            hunks: vec![DiffHunk {
                original_start: 0, original_length: 0,
                modified_start: 1, modified_length: 2,
                lines: vec![
                    DiffLine { content: "+a".into(), kind: DiffLineKind::Added },
                    DiffLine { content: "+b".into(), kind: DiffLineKind::Added },
                ],
            }],
        });
        let paths = model.affected_paths();
        assert_eq!(paths, vec!["src/main.rs"]);
        assert_eq!(model.net_change(), 2);
    }

    #[test]
    fn diffs_by_extension_filters_correctly() {
        let mut model = MultiDiffModel::new();
        model.add_diff(FileDiff {
            original_uri: Some("a.rs".into()),
            modified_uri: Some("a.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![],
        });
        model.add_diff(FileDiff {
            original_uri: Some("b.ts".into()),
            modified_uri: Some("b.ts".into()),
            kind: DiffKind::Modified,
            hunks: vec![],
        });
        assert_eq!(model.diffs_by_extension("rs").len(), 1);
        assert_eq!(model.diffs_by_extension("ts").len(), 1);
        assert_eq!(model.diffs_by_extension("py").len(), 0);
    }

    #[test]
    fn largest_diff_returns_most_changed() {
        let mut model = MultiDiffModel::new();
        model.add_diff(FileDiff {
            original_uri: Some("small.rs".into()),
            modified_uri: Some("small.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![DiffHunk {
                original_start: 1, original_length: 1,
                modified_start: 1, modified_length: 1,
                lines: vec![DiffLine { content: "+x".into(), kind: DiffLineKind::Added }],
            }],
        });
        model.add_diff(FileDiff {
            original_uri: Some("big.rs".into()),
            modified_uri: Some("big.rs".into()),
            kind: DiffKind::Modified,
            hunks: vec![DiffHunk {
                original_start: 1, original_length: 5,
                modified_start: 1, modified_length: 5,
                lines: vec![
                    DiffLine { content: "+a".into(), kind: DiffLineKind::Added },
                    DiffLine { content: "+b".into(), kind: DiffLineKind::Added },
                    DiffLine { content: "+c".into(), kind: DiffLineKind::Added },
                ],
            }],
        });
        let largest = model.largest_diff().unwrap();
        assert_eq!(largest.display_path(), "big.rs");
    }

    // -- MultiDiffStatsSummary tests --

    #[test]
    fn stats_summary_from_model() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Added, 2));
        model.add_diff(make_diff(DiffKind::Modified, 1));
        let s = MultiDiffStatsSummary::from_model(&model);
        assert_eq!(s.total_files, 2);
        assert_eq!(s.files_added, 1);
        assert_eq!(s.files_modified, 1);
        assert!(s.short_summary().contains("2 files"));
    }

    #[test]
    fn stats_summary_net_change() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Added, 1));
        let s = MultiDiffStatsSummary::from_model(&model);
        let net = s.net_change();
        assert!(net >= 0 || net < 0);
    }

    // -- MultiDiffCollapse tests --

    #[test]
    fn collapse_toggle() {
        let mut c = MultiDiffCollapse::new(3);
        assert!(!c.is_collapsed(0));
        assert_eq!(c.toggle(0), Some(true));
        assert!(c.is_collapsed(0));
        assert_eq!(c.expanded_count(), 2);
        assert_eq!(c.collapsed_count(), 1);
    }

    #[test]
    fn collapse_all_expand_all() {
        let mut c = MultiDiffCollapse::new(5);
        c.collapse_all();
        assert_eq!(c.collapsed_count(), 5);
        c.expand_all();
        assert_eq!(c.expanded_count(), 5);
    }

    #[test]
    fn collapse_out_of_bounds() {
        let mut c = MultiDiffCollapse::new(2);
        assert_eq!(c.toggle(10), None);
        assert!(!c.is_collapsed(10));
    }

    // -- MultiDiffHunkActions tests --

    #[test]
    fn hunk_actions_lifecycle() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Modified, 3));
        let mut actions = MultiDiffHunkActions::from_model(&model);
        assert_eq!(actions.pending_count(), 3);
        assert!(!actions.is_all_resolved());
        actions.set_action(0, 0, HunkAction::Accepted);
        actions.set_action(0, 1, HunkAction::Rejected);
        actions.set_action(0, 2, HunkAction::Accepted);
        assert!(actions.is_all_resolved());
        assert_eq!(actions.accepted_count(), 2);
    }

    #[test]
    fn hunk_actions_accept_all_in_file() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Modified, 2));
        model.add_diff(make_diff(DiffKind::Added, 1));
        let mut actions = MultiDiffHunkActions::from_model(&model);
        actions.accept_all_in_file(0);
        assert_eq!(actions.get_action(0, 0), HunkAction::Accepted);
        assert_eq!(actions.get_action(0, 1), HunkAction::Accepted);
        assert_eq!(actions.get_action(1, 0), HunkAction::Pending);
    }

    #[test]
    fn hunk_actions_reject_all() {
        let mut model = MultiDiffModel::new();
        model.add_diff(make_diff(DiffKind::Modified, 2));
        let mut actions = MultiDiffHunkActions::from_model(&model);
        actions.reject_all_in_file(0);
        assert_eq!(actions.pending_count(), 0);
        assert_eq!(actions.accepted_count(), 0);
    }
}

// ── Side-by-side Diff View (ratatui) ──

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

/// Kind of line in the diff view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffViewLineKind {
    Unchanged,
    Added,
    Deleted,
    Modified,
    /// Padding for alignment.
    Empty,
}

/// A single line in one side of the diff view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffViewLine {
    pub content: String,
    pub line_number: Option<usize>,
    pub kind: DiffViewLineKind,
}

/// Renders a side-by-side diff view of two texts.
pub struct DiffView {
    pub left_lines: Vec<DiffViewLine>,
    pub right_lines: Vec<DiffViewLine>,
    pub scroll_offset: usize,
    pub title_left: String,
    pub title_right: String,
}

impl DiffView {
    /// Create a diff view from two texts.
    pub fn from_texts(left: &str, right: &str, title_left: &str, title_right: &str) -> Self {
        let diff = vsedit_diff::compute_line_diff(left, right);
        let left_src: Vec<&str> = left.lines().collect();
        let right_src: Vec<&str> = right.lines().collect();

        let mut left_lines = Vec::new();
        let mut right_lines = Vec::new();
        let mut left_pos: usize = 0;
        let mut right_pos: usize = 0;

        for change in &diff.changes {
            let orig_start = (change.original_start as usize).saturating_sub(1);
            let mod_start = (change.modified_start as usize).saturating_sub(1);

            // Emit unchanged lines before this change
            while left_pos < orig_start && left_pos < left_src.len() {
                left_lines.push(DiffViewLine {
                    content: left_src[left_pos].to_string(),
                    line_number: Some(left_pos + 1),
                    kind: DiffViewLineKind::Unchanged,
                });
                right_lines.push(DiffViewLine {
                    content: right_src[right_pos].to_string(),
                    line_number: Some(right_pos + 1),
                    kind: DiffViewLineKind::Unchanged,
                });
                left_pos += 1;
                right_pos += 1;
            }

            let ol = change.original_length as usize;
            let ml = change.modified_length as usize;

            match change.kind {
                vsedit_diff::DiffChangeKind::Delete => {
                    for i in 0..ol {
                        let idx = orig_start + i;
                        left_lines.push(DiffViewLine {
                            content: left_src.get(idx).unwrap_or(&"").to_string(),
                            line_number: Some(idx + 1),
                            kind: DiffViewLineKind::Deleted,
                        });
                        right_lines.push(DiffViewLine {
                            content: String::new(),
                            line_number: None,
                            kind: DiffViewLineKind::Empty,
                        });
                    }
                    left_pos = orig_start + ol;
                }
                vsedit_diff::DiffChangeKind::Insert => {
                    for i in 0..ml {
                        let idx = mod_start + i;
                        left_lines.push(DiffViewLine {
                            content: String::new(),
                            line_number: None,
                            kind: DiffViewLineKind::Empty,
                        });
                        right_lines.push(DiffViewLine {
                            content: right_src.get(idx).unwrap_or(&"").to_string(),
                            line_number: Some(idx + 1),
                            kind: DiffViewLineKind::Added,
                        });
                    }
                    right_pos = mod_start + ml;
                }
                vsedit_diff::DiffChangeKind::Change => {
                    let max = ol.max(ml);
                    for i in 0..max {
                        if i < ol {
                            let idx = orig_start + i;
                            left_lines.push(DiffViewLine {
                                content: left_src.get(idx).unwrap_or(&"").to_string(),
                                line_number: Some(idx + 1),
                                kind: DiffViewLineKind::Modified,
                            });
                        } else {
                            left_lines.push(DiffViewLine {
                                content: String::new(),
                                line_number: None,
                                kind: DiffViewLineKind::Empty,
                            });
                        }
                        if i < ml {
                            let idx = mod_start + i;
                            right_lines.push(DiffViewLine {
                                content: right_src.get(idx).unwrap_or(&"").to_string(),
                                line_number: Some(idx + 1),
                                kind: DiffViewLineKind::Modified,
                            });
                        } else {
                            right_lines.push(DiffViewLine {
                                content: String::new(),
                                line_number: None,
                                kind: DiffViewLineKind::Empty,
                            });
                        }
                    }
                    left_pos = orig_start + ol;
                    right_pos = mod_start + ml;
                }
            }
        }

        // Remaining unchanged lines
        while left_pos < left_src.len() && right_pos < right_src.len() {
            left_lines.push(DiffViewLine {
                content: left_src[left_pos].to_string(),
                line_number: Some(left_pos + 1),
                kind: DiffViewLineKind::Unchanged,
            });
            right_lines.push(DiffViewLine {
                content: right_src[right_pos].to_string(),
                line_number: Some(right_pos + 1),
                kind: DiffViewLineKind::Unchanged,
            });
            left_pos += 1;
            right_pos += 1;
        }

        Self {
            left_lines,
            right_lines,
            scroll_offset: 0,
            title_left: title_left.to_string(),
            title_right: title_right.to_string(),
        }
    }

    /// Render the diff view using ratatui.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let halves = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

        self.render_panel(&self.left_lines, &self.title_left, halves[0], buf);
        self.render_panel(&self.right_lines, &self.title_right, halves[1], buf);
    }

    fn render_panel(&self, lines: &[DiffViewLine], title: &str, area: Rect, buf: &mut Buffer) {
        let visible_height = area.height.saturating_sub(2) as usize; // borders
        let start = self.scroll_offset;
        let end = (start + visible_height).min(lines.len());
        let visible: Vec<Line<'_>> = lines[start..end]
            .iter()
            .map(|dl| {
                let gutter = match dl.line_number {
                    Some(n) => format!("{:>4} ", n),
                    None => "     ".to_string(),
                };
                let style = match dl.kind {
                    DiffViewLineKind::Added => Style::default().bg(Color::Green),
                    DiffViewLineKind::Deleted => Style::default().bg(Color::Red),
                    DiffViewLineKind::Modified => Style::default().bg(Color::Yellow),
                    DiffViewLineKind::Empty => Style::default().bg(Color::DarkGray),
                    DiffViewLineKind::Unchanged => Style::default(),
                };
                Line::from(vec![
                    Span::raw(gutter),
                    Span::styled(&dl.content, style),
                ])
            })
            .collect();
        let paragraph = Paragraph::new(visible)
            .block(Block::default().borders(Borders::ALL).title(title.to_string()));
        Widget::render(paragraph, area, buf);
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    pub fn scroll_down(&mut self, n: usize) {
        let max = self.total_lines();
        self.scroll_offset = (self.scroll_offset + n).min(max.saturating_sub(1));
    }

    pub fn total_lines(&self) -> usize {
        self.left_lines.len().max(self.right_lines.len())
    }
}


// ---------------------------------------------------------------------------
// vsedit-multidiff: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultidiffXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl MultidiffXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for MultidiffXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct MultidiffXRegistry {
    entries: Vec<MultidiffXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl MultidiffXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: MultidiffXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&MultidiffXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut MultidiffXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<MultidiffXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&MultidiffXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&MultidiffXConfig> {
        let mut sorted: Vec<&MultidiffXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&MultidiffXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> MultidiffXIterator<'_> {
        MultidiffXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct MultidiffXIterator<'a> {
    inner: std::slice::Iter<'a, MultidiffXConfig>,
}

impl<'a> Iterator for MultidiffXIterator<'a> {
    type Item = &'a MultidiffXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct MultidiffXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl MultidiffXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct MultidiffXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl MultidiffXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &MultidiffXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &MultidiffXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &MultidiffXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for MultidiffXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct MultidiffXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl MultidiffXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &MultidiffXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &MultidiffXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for MultidiffXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for multidiff
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaMultidiffRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaMultidiffRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaMultidiffCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaMultidiffCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaMultidiffCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xg_20: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg20Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg20Graph {
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

impl Default for Xg20Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_20: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg20Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg20Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg20Heap<T>) {
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

impl<T: Ord> Default for Xg20Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A double-ended queue backed by a ring buffer (variant 125).
pub struct Xi125Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi125Deque<T> {
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
pub struct Xi125Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi125Interval {
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

/// A simple interval tree (variant 125).
pub struct Xi125IntervalTree {
    xi_intervals: Vec<Xi125Interval>,
}

impl Xi125IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi125Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi125Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi125Interval) -> Vec<&Xi125Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi125Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi125Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi125Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi125Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi125Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi125Interval> = Vec::new();
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


/// Rope data structure for efficient large text manipulation (xl_125).
#[derive(Debug, Clone)]
pub struct Xl125Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl125Rope {
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

/// Suffix array for efficient string searching (xl_125).
#[derive(Debug, Clone)]
pub struct Xl125SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl125SuffixArray {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 125.
pub struct Xn125Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn125Fenwick {
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

// ----- AVL tree map — crate 125 -----

#[derive(Debug, Clone)]
struct Xn125AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn125AvlNode<K, V>>>,
    right: Option<Box<Xn125AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 125.
#[derive(Debug, Clone)]
pub struct Xn125AVL<K, V> {
    root: Option<Box<Xn125AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn125AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn125AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn125AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn125AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn125AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn125AvlNode<K, V>>) -> Box<Xn125AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn125AvlNode<K, V>>) -> Box<Xn125AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn125AvlNode<K, V>>) -> Box<Xn125AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn125AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn125AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn125AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn125AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn125AvlNode<K, V>>) -> &Xn125AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn125AvlNode<K, V>>) -> (Box<Xn125AvlNode<K, V>>, Option<Box<Xn125AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn125AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn125AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn125AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn125AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn125AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn125AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn125AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

#[cfg(test)]
mod diff_view_tests {
    use super::*;

    #[test]
    fn diff_view_identical_files() {
        let text = "line1\nline2\nline3\n";
        let view = DiffView::from_texts(text, text, "a.txt", "b.txt");
        assert_eq!(view.total_lines(), 3);
        for line in &view.left_lines {
            assert_eq!(line.kind, DiffViewLineKind::Unchanged);
        }
        for line in &view.right_lines {
            assert_eq!(line.kind, DiffViewLineKind::Unchanged);
        }
    }

    #[test]
    fn diff_view_added_lines() {
        let left = "a\nc\n";
        let right = "a\nb\nc\n";
        let view = DiffView::from_texts(left, right, "old", "new");
        // Should have an Added line on the right and Empty on the left
        let added = view.right_lines.iter().any(|l| l.kind == DiffViewLineKind::Added);
        let empty = view.left_lines.iter().any(|l| l.kind == DiffViewLineKind::Empty);
        assert!(added);
        assert!(empty);
    }

    #[test]
    fn diff_view_deleted_lines() {
        let left = "a\nb\nc\n";
        let right = "a\nc\n";
        let view = DiffView::from_texts(left, right, "old", "new");
        let deleted = view.left_lines.iter().any(|l| l.kind == DiffViewLineKind::Deleted);
        let empty = view.right_lines.iter().any(|l| l.kind == DiffViewLineKind::Empty);
        assert!(deleted);
        assert!(empty);
    }

    #[test]
    fn diff_view_modified_lines() {
        let left = "a\nb\n";
        let right = "a\nB\n";
        let view = DiffView::from_texts(left, right, "old", "new");
        let left_modified = view.left_lines.iter().any(|l| l.kind == DiffViewLineKind::Modified);
        let right_modified = view.right_lines.iter().any(|l| l.kind == DiffViewLineKind::Modified);
        assert!(left_modified);
        assert!(right_modified);
    }

    #[test]
    fn diff_view_empty_files() {
        let view = DiffView::from_texts("", "", "a", "b");
        assert_eq!(view.total_lines(), 0);
        assert!(view.left_lines.is_empty());
        assert!(view.right_lines.is_empty());
    }

    #[test]
    fn diff_view_scroll_up_down() {
        let left = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";
        let right = "1\n2\nX\n4\n5\n6\n7\n8\n9\n10\n";
        let mut view = DiffView::from_texts(left, right, "l", "r");
        assert_eq!(view.scroll_offset, 0);

        view.scroll_down(3);
        assert_eq!(view.scroll_offset, 3);

        view.scroll_up(2);
        assert_eq!(view.scroll_offset, 1);

        view.scroll_up(100);
        assert_eq!(view.scroll_offset, 0);
    }

    #[test]
    fn diff_view_scroll_down_clamped() {
        let view_text = "a\nb\n";
        let mut view = DiffView::from_texts(view_text, view_text, "l", "r");
        view.scroll_down(1000);
        assert!(view.scroll_offset <= view.total_lines());
    }

    #[test]
    fn diff_view_render_does_not_panic() {
        let left = "hello\nworld\n";
        let right = "hello\nrust\n";
        let view = DiffView::from_texts(left, right, "original.txt", "modified.txt");
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
    }

    #[test]
    fn diff_view_render_empty_does_not_panic() {
        let view = DiffView::from_texts("", "", "a", "b");
        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
    }

    #[test]
    fn diff_view_line_numbers_correct() {
        let left = "a\nb\nc\n";
        let right = "a\nc\n";
        let view = DiffView::from_texts(left, right, "l", "r");
        // First line on both sides should be line 1
        assert_eq!(view.left_lines[0].line_number, Some(1));
        assert_eq!(view.right_lines[0].line_number, Some(1));
        // Empty padding lines have no line number
        let empties: Vec<_> = view.right_lines.iter().filter(|l| l.kind == DiffViewLineKind::Empty).collect();
        for e in &empties {
            assert_eq!(e.line_number, None);
        }
    }

    #[test]
    fn diff_view_titles_stored() {
        let view = DiffView::from_texts("a\n", "a\n", "left.rs", "right.rs");
        assert_eq!(view.title_left, "left.rs");
        assert_eq!(view.title_right, "right.rs");
    }

    // --- MultiDiffSearch tests ---

    #[test]
    fn search_finds_matches_across_files() {
        let mut s = MultiDiffSearch::new();
        s.add_diff("a.rs", "fn main() {}\nlet x = 1;");
        s.add_diff("b.rs", "fn helper() {}\nfn aux() {}");
        let results = s.search("fn");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].file_path, "a.rs");
        assert_eq!(results[1].file_path, "b.rs");
    }

    #[test]
    fn search_empty_query_returns_nothing() {
        let mut s = MultiDiffSearch::new();
        s.add_diff("a.rs", "hello world");
        assert_eq!(s.search("").len(), 0);
    }

    #[test]
    fn search_count_and_file_count() {
        let mut s = MultiDiffSearch::new();
        s.add_diff("x.rs", "aaa\naab\naaa");
        s.add_diff("y.rs", "bbb");
        assert_eq!(s.file_count(), 2);
        assert_eq!(s.search_count("aa"), 3);
    }

    #[test]
    fn search_result_display() {
        let r = MultiDiffSearchResult {
            file_path: "f.rs".into(),
            line_num: 3,
            line_text: "let x = 42;".into(),
            match_start: 4,
            match_end: 5,
        };
        let disp = format!("{r}");
        assert!(disp.contains("f.rs:3"));
    }

    // --- MultiDiffExport tests ---

    #[test]
    fn export_unified_diff_headers() {
        let mut e = MultiDiffExport::new();
        e.add_file_diff("main.rs", &["a", "b"], &["a", "c"]);
        let diff = e.to_unified_diff();
        assert!(diff.contains("--- a/main.rs"));
        assert!(diff.contains("+++ b/main.rs"));
        assert!(diff.contains("@@ -1,2 +1,2 @@"));
    }

    #[test]
    fn export_file_count() {
        let mut e = MultiDiffExport::new();
        assert_eq!(e.file_count(), 0);
        e.add_file_diff("a.rs", &["x"], &["y"]);
        e.add_file_diff("b.rs", &["x"], &["y"]);
        assert_eq!(e.file_count(), 2);
    }

    #[test]
    fn export_display() {
        let e = MultiDiffExport::new();
        assert_eq!(format!("{e}"), "MultiDiffExport(0 files)");
    }

    // --- MultiDiffMerge tests ---

    #[test]
    fn merge_adjacent_hunks() {
        let mut m = MultiDiffMerge::new();
        m.add_hunk("f.rs", 1, 3, 3, "abc");
        m.add_hunk("f.rs", 5, 2, 2, "de");
        let merged = m.merge_adjacent(2);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].file, "f.rs");
        assert_eq!(merged[0].start, 1);
    }

    #[test]
    fn merge_non_adjacent_hunks() {
        let mut m = MultiDiffMerge::new();
        m.add_hunk("f.rs", 1, 2, 2, "ab");
        m.add_hunk("f.rs", 20, 2, 2, "xy");
        let merged = m.merge_adjacent(1);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_hunk_count() {
        let mut m = MultiDiffMerge::new();
        m.add_hunk("a.rs", 1, 1, 1, "x");
        m.add_hunk("a.rs", 5, 1, 1, "y");
        m.add_hunk("b.rs", 1, 1, 1, "z");
        assert_eq!(m.hunk_count(), 3);
    }

    // --- MultiDiffFilter tests ---

    #[test]
    fn filter_by_kind_returns_correct_files() {
        let mut f = MultiDiffFilter::new();
        f.add_change("a.rs", DiffChangeKind::Added);
        f.add_change("b.rs", DiffChangeKind::Modified);
        f.add_change("c.rs", DiffChangeKind::Added);
        let added = f.files_with_kind(DiffChangeKind::Added);
        assert_eq!(added, vec!["a.rs", "c.rs"]);
    }

    #[test]
    fn filter_total_and_count_by_kind() {
        let mut f = MultiDiffFilter::new();
        f.add_change("a.rs", DiffChangeKind::Removed);
        f.add_change("b.rs", DiffChangeKind::Removed);
        f.add_change("c.rs", DiffChangeKind::Renamed);
        assert_eq!(f.total_files(), 3);
        assert_eq!(f.count_by_kind(DiffChangeKind::Removed), 2);
        assert_eq!(f.count_by_kind(DiffChangeKind::Renamed), 1);
        assert_eq!(f.count_by_kind(DiffChangeKind::Added), 0);
    }

    #[test]
    fn filter_display() {
        let mut f = MultiDiffFilter::new();
        f.add_change("x.rs", DiffChangeKind::Modified);
        assert_eq!(format!("{f}"), "MultiDiffFilter(1 files)");
    }

    #[test]
    fn diff_change_kind_display() {
        assert_eq!(format!("{}", DiffChangeKind::Added), "Added");
        assert_eq!(format!("{}", DiffChangeKind::Removed), "Removed");
        assert_eq!(format!("{}", DiffChangeKind::Modified), "Modified");
        assert_eq!(format!("{}", DiffChangeKind::Renamed), "Renamed");
    }

    #[test]
    fn multidiff_x_config_new() {
        let c = MultidiffXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn multidiff_x_config_builder() {
        let c = MultidiffXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn multidiff_x_config_display() {
        let c = MultidiffXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn multidiff_x_registry_insert_get() {
        let mut reg = MultidiffXRegistry::new();
        reg.insert(MultidiffXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn multidiff_x_registry_duplicate() {
        let mut reg = MultidiffXRegistry::new();
        reg.insert(MultidiffXConfig::new("a")).unwrap();
        assert!(reg.insert(MultidiffXConfig::new("a")).is_err());
    }

    #[test]
    fn multidiff_x_registry_remove() {
        let mut reg = MultidiffXRegistry::new();
        reg.insert(MultidiffXConfig::new("a")).unwrap();
        reg.insert(MultidiffXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn multidiff_x_registry_active_entries() {
        let mut reg = MultidiffXRegistry::new();
        reg.insert(MultidiffXConfig::new("a")).unwrap();
        reg.insert(MultidiffXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn multidiff_x_registry_by_weight() {
        let mut reg = MultidiffXRegistry::new();
        reg.insert(MultidiffXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(MultidiffXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn multidiff_x_registry_tags() {
        let mut reg = MultidiffXRegistry::new();
        reg.insert(MultidiffXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(MultidiffXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn multidiff_x_registry_total_weight() {
        let mut reg = MultidiffXRegistry::new();
        reg.insert(MultidiffXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(MultidiffXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn multidiff_x_registry_iterator() {
        let mut reg = MultidiffXRegistry::new();
        reg.insert(MultidiffXConfig::new("a")).unwrap();
        reg.insert(MultidiffXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn multidiff_x_cache_put_get() {
        let mut cache = MultidiffXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn multidiff_x_cache_eviction() {
        let mut cache = MultidiffXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn multidiff_x_cache_lru_order() {
        let mut cache = MultidiffXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn multidiff_x_cache_most_least_recent() {
        let mut cache = MultidiffXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn multidiff_x_formatter_entry() {
        let e = MultidiffXConfig::new("k").with_value("v");
        let fmt = MultidiffXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn multidiff_x_formatter_summary() {
        let mut reg = MultidiffXRegistry::new();
        reg.insert(MultidiffXConfig::new("a").with_weight(5)).unwrap();
        let fmt = MultidiffXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn multidiff_x_validator_valid() {
        let v = MultidiffXValidator::new();
        let c = MultidiffXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn multidiff_x_validator_empty_key() {
        let v = MultidiffXValidator::new();
        let c = MultidiffXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn multidiff_x_validator_require_value() {
        let v = MultidiffXValidator::new().require_value(true);
        let c = MultidiffXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn multidiff_x_validator_allowed_tags() {
        let v = MultidiffXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = MultidiffXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn multidiff_x_validator_validate_all() {
        let v = MultidiffXValidator::new();
        let mut reg = MultidiffXRegistry::new();
        reg.insert(MultidiffXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for multidiff
    #[test]
    fn xa_multidiff_ring_new() {
        let rb = super::XaMultidiffRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_multidiff_ring_push_len() {
        let mut rb = super::XaMultidiffRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_multidiff_ring_wrap() {
        let mut rb = super::XaMultidiffRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_multidiff_ring_mean_empty() {
        let rb = super::XaMultidiffRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_multidiff_ring_mean_values() {
        let mut rb = super::XaMultidiffRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_multidiff_ring_min_max() {
        let mut rb = super::XaMultidiffRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_multidiff_ring_iter() {
        let mut rb = super::XaMultidiffRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_multidiff_counter_new() {
        let c = super::XaMultidiffCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_multidiff_counter_inc() {
        let mut c = super::XaMultidiffCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_multidiff_counter_inc_by() {
        let mut c = super::XaMultidiffCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_multidiff_counter_reset() {
        let mut c = super::XaMultidiffCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_multidiff_counter_clear() {
        let mut c = super::XaMultidiffCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_multidiff_counter_default() {
        let c = super::XaMultidiffCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 126 ----

    #[test]
    fn xc_126_pool_new_empty() {
        let pool: super::Xc126Pool<i32> = super::Xc126Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_126_pool_release_acquire() {
        let mut pool = super::Xc126Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_126_pool_acquire_empty() {
        let mut pool: super::Xc126Pool<i32> = super::Xc126Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_126_pool_full() {
        let mut pool = super::Xc126Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_126_pool_drain() {
        let mut pool = super::Xc126Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_126_pool_stats() {
        let mut pool = super::Xc126Pool::new(8);
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
    fn xc_126_pool_clear() {
        let mut pool = super::Xc126Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_126_pool_shrink() {
        let mut pool = super::Xc126Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_126_pool_default() {
        let pool: super::Xc126Pool<String> = super::Xc126Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_126_pool_extend() {
        let mut pool = super::Xc126Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_126_pool_retain() {
        let mut pool = super::Xc126Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_126_scheduler_round_robin() {
        let mut sched = super::Xc126Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_126_scheduler_empty() {
        let mut sched = super::Xc126Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_126_scheduler_reset() {
        let mut sched = super::Xc126Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_126_scheduler_add_remove() {
        let mut sched = super::Xc126Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_126_scheduler_targets() {
        let sched = super::Xc126Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_126_hash_empty() {
        assert_eq!(super::xc_126_hash(b""), 5381);
    }

    #[test]
    fn xc_126_hash_data() {
        let h = super::xc_126_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_126_hash(b"hello"), h);
    }

    #[test]
    fn xc_126_reverse_str() {
        assert_eq!(super::xc_126_reverse("abc"), "cba");
        assert_eq!(super::xc_126_reverse(""), "");
    }


    // --- xd_96 deepening tests ---

    #[test]
    fn xd_96_sm_initial_state() {
        let sm = Xd96StateMachine::new();
        assert_eq!(sm.current_state(), Xd96State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_96_sm_valid_idle_to_running() {
        let mut sm = Xd96StateMachine::new();
        assert!(sm.transition(Xd96State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd96State::Running);
    }

    #[test]
    fn xd_96_sm_valid_running_to_paused() {
        let mut sm = Xd96StateMachine::new();
        sm.transition(Xd96State::Running).unwrap();
        assert!(sm.transition(Xd96State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd96State::Paused);
    }

    #[test]
    fn xd_96_sm_valid_running_to_done() {
        let mut sm = Xd96StateMachine::new();
        sm.transition(Xd96State::Running).unwrap();
        assert!(sm.transition(Xd96State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd96State::Done);
    }

    #[test]
    fn xd_96_sm_valid_paused_to_running() {
        let mut sm = Xd96StateMachine::new();
        sm.transition(Xd96State::Running).unwrap();
        sm.transition(Xd96State::Paused).unwrap();
        assert!(sm.transition(Xd96State::Running).is_ok());
    }

    #[test]
    fn xd_96_sm_valid_done_to_idle() {
        let mut sm = Xd96StateMachine::new();
        sm.transition(Xd96State::Running).unwrap();
        sm.transition(Xd96State::Done).unwrap();
        assert!(sm.transition(Xd96State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd96State::Idle);
    }

    #[test]
    fn xd_96_sm_invalid_idle_to_done() {
        let mut sm = Xd96StateMachine::new();
        assert!(sm.transition(Xd96State::Done).is_err());
    }

    #[test]
    fn xd_96_sm_invalid_idle_to_paused() {
        let mut sm = Xd96StateMachine::new();
        assert!(sm.transition(Xd96State::Paused).is_err());
    }

    #[test]
    fn xd_96_sm_history_tracking() {
        let mut sm = Xd96StateMachine::new();
        sm.transition(Xd96State::Running).unwrap();
        sm.transition(Xd96State::Paused).unwrap();
        sm.transition(Xd96State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd96State::Idle);
        assert_eq!(sm.history()[0].to, Xd96State::Running);
        assert_eq!(sm.history()[1].from, Xd96State::Running);
        assert_eq!(sm.history()[2].to, Xd96State::Done);
    }

    #[test]
    fn xd_96_sm_serialize_deserialize() {
        let mut sm = Xd96StateMachine::new();
        sm.transition(Xd96State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd96StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd96State::Running));
    }

    #[test]
    fn xd_96_sm_deserialize_invalid() {
        assert_eq!(Xd96StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_96_sm_reset() {
        let mut sm = Xd96StateMachine::new();
        sm.transition(Xd96State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd96State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_96_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd96EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd96Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_96_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd96EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd96Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd96Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_96_bus_unsubscribe() {
        let mut bus = Xd96EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_96_event_kind_and_payload() {
        let e = Xd96Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd96Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_96_bus_clear_history() {
        let mut bus = Xd96EventBus::new();
        bus.publish(Xd96Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_96_sm_step_counter_increments() {
        let mut sm = Xd96StateMachine::new();
        sm.transition(Xd96State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd96State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_20 graph tests ------------------------------------------------

    #[test]
    fn xg_20_graph_empty() {
        let g = super::Xg20Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_20_graph_add_node() {
        let mut g = super::Xg20Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_20_graph_add_edge() {
        let mut g = super::Xg20Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_20_graph_neighbors() {
        let mut g = super::Xg20Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_20_graph_has_path() {
        let mut g = super::Xg20Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_20_graph_self_path() {
        let g = super::Xg20Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_20_graph_topo_sort() {
        let mut g = super::Xg20Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_20_graph_cycle_detect_false() {
        let mut g = super::Xg20Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_20_graph_cycle_detect_true() {
        let mut g = super::Xg20Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_20 heap tests -------------------------------------------------

    #[test]
    fn xg_20_heap_empty() {
        let h: super::Xg20Heap<i32> = super::Xg20Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_20_heap_push_pop() {
        let mut h = super::Xg20Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_20_heap_peek() {
        let mut h = super::Xg20Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_20_heap_drain_sorted() {
        let mut h = super::Xg20Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_20_heap_merge() {
        let mut a = super::Xg20Heap::new();
        let mut b = super::Xg20Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_20_heap_default() {
        let h: super::Xg20Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_20_graph_default() {
        let g: super::Xg20Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh125_skip_insert_contains() {
        let mut sl = super::Xh125SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh125_skip_remove() {
        let mut sl = super::Xh125SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh125_skip_len() {
        let mut sl = super::Xh125SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh125_skip_range_query() {
        let mut sl = super::Xh125SkipList::xh_new(4);
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
    fn xh125_skip_floor_ceiling() {
        let mut sl = super::Xh125SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh125_skip_rank() {
        let mut sl = super::Xh125SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh125_skip_empty() {
        let sl = super::Xh125SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh125_skip_duplicates() {
        let mut sl = super::Xh125SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh125_bitset_set_test() {
        let mut bs = super::Xh125BitSet::xh_new(256);
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
    fn xh125_bitset_clear_count() {
        let mut bs = super::Xh125BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh125_bitset_and_or_xor() {
        let mut a = super::Xh125BitSet::xh_new(128);
        let mut b = super::Xh125BitSet::xh_new(128);
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
    fn xh125_bitset_iter_ones() {
        let mut bs = super::Xh125BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh125_bitset_first_last() {
        let mut bs = super::Xh125BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh125_bitset_empty() {
        let bs = super::Xh125BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi125_deque_push_pop_back() {
        let mut dq = super::Xi125Deque::xi_new(4);
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
    fn xi125_deque_push_pop_front() {
        let mut dq = super::Xi125Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi125_deque_mixed_ops() {
        let mut dq = super::Xi125Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi125_deque_get_and_split() {
        let mut dq = super::Xi125Deque::xi_new(8);
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
    fn xi125_deque_rotate_left() {
        let mut dq = super::Xi125Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi125_deque_rotate_right() {
        let mut dq = super::Xi125Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi125_deque_grow() {
        let mut dq = super::Xi125Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi125_deque_empty() {
        let dq = super::Xi125Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi125_interval_tree_insert_query() {
        let mut tree = super::Xi125IntervalTree::xi_new();
        tree.xi_insert(super::Xi125Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi125Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi125Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi125_interval_tree_overlap() {
        let mut tree = super::Xi125IntervalTree::xi_new();
        tree.xi_insert(super::Xi125Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi125Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi125Interval::xi_new(12, 20));
        let q = super::Xi125Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi125_interval_tree_remove() {
        let mut tree = super::Xi125IntervalTree::xi_new();
        tree.xi_insert(super::Xi125Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi125Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi125_interval_tree_gaps() {
        let mut tree = super::Xi125IntervalTree::xi_new();
        tree.xi_insert(super::Xi125Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi125Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi125Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi125Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi125Interval::xi_new(8, 10));
    }

    #[test]
    fn xi125_interval_tree_merge() {
        let mut tree = super::Xi125IntervalTree::xi_new();
        tree.xi_insert(super::Xi125Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi125Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi125Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi125Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi125Interval::xi_new(10, 15));
    }

    #[test]
    fn xi125_interval_tree_all() {
        let mut tree = super::Xi125IntervalTree::xi_new();
        tree.xi_insert(super::Xi125Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi125Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi125_interval_tree_empty() {
        let tree = super::Xi125IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi125_interval_tree_contains_point() {
        let iv = super::Xi125Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 125) ---

    #[test]
    fn xj_125_uf_make_and_find() {
        let mut uf = super::Xj125UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_125_uf_union_connected() {
        let mut uf = super::Xj125UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_125_uf_component_count() {
        let mut uf = super::Xj125UnionFind::xj_new();
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
    fn xj_125_uf_component_size() {
        let mut uf = super::Xj125UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_125_uf_largest_component() {
        let mut uf = super::Xj125UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_125_uf_many_elements() {
        let mut uf = super::Xj125UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_125_uf_separate_components() {
        let mut uf = super::Xj125UnionFind::xj_new();
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
    fn xj_125_uf_path_compression() {
        let mut uf = super::Xj125UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_125_bt_insert_get() {
        let mut bt = super::Xj125BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_125_bt_contains_len() {
        let mut bt = super::Xj125BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_125_bt_replace() {
        let mut bt = super::Xj125BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_125_bt_remove() {
        let mut bt = super::Xj125BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_125_bt_keys_values() {
        let mut bt = super::Xj125BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_125_bt_range() {
        let mut bt = super::Xj125BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_125_bt_min_max() {
        let mut bt = super::Xj125BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_125_bt_many_inserts() {
        let mut bt = super::Xj125BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_125 segment tree tests ---

    #[test]
    fn xk_125_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk125SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_125_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk125SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_125_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk125SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_125_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk125SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_125_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk125SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_125_st_single_element() {
        let data = vec![42];
        let st = super::Xk125SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_125_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk125SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_125_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk125SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_125 disjoint intervals tests ---

    #[test]
    fn xk_125_di_add_and_count() {
        let mut di = super::Xk125DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_125_di_merge_overlap() {
        let mut di = super::Xk125DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_125_di_contains() {
        let mut di = super::Xk125DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_125_di_remove() {
        let mut di = super::Xk125DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_125_di_covered_length() {
        let mut di = super::Xk125DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_125_di_gaps() {
        let mut di = super::Xk125DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_125_di_merge_adjacent() {
        let mut di = super::Xk125DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_125_di_empty() {
        let di = super::Xk125DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_125_rope_new_empty() {
        let rope = super::Xl125Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_125_rope_from_str() {
        let rope = super::Xl125Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_125_rope_insert_at() {
        let mut rope = super::Xl125Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_125_rope_delete_range() {
        let mut rope = super::Xl125Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_125_rope_char_at() {
        let rope = super::Xl125Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_125_rope_split_concat() {
        let rope = super::Xl125Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_125_rope_line_count() {
        let rope = super::Xl125Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_125_rope_line_at() {
        let rope = super::Xl125Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_125_sa_build_and_search() {
        let sa = super::Xl125SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_125_sa_count() {
        let sa = super::Xl125SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_125_sa_longest_repeated() {
        let sa = super::Xl125SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_125_sa_all_positions() {
        let sa = super::Xl125SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_125_sa_len() {
        let sa = super::Xl125SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_125_sa_empty() {
        let sa = super::Xl125SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_125_rope_slice() {
        let rope = super::Xl125Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_125_sa_search_start() {
        let sa = super::Xl125SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_125_sparse_set_get() {
        let mut m = super::Xm125MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_125_sparse_row_col() {
        let mut m = super::Xm125MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_125_sparse_transpose() {
        let mut m = super::Xm125MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_125_sparse_multiply_vec() {
        let mut m = super::Xm125MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_125_sparse_nnz_density() {
        let mut m = super::Xm125MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_125_sparse_clear() {
        let mut m = super::Xm125MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_125_sparse_overwrite_zero() {
        let mut m = super::Xm125MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_125_tokenizer_basic() {
        let t = super::Xm125Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_125_tokenizer_count() {
        let t = super::Xm125Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_125_tokenizer_unique() {
        let t = super::Xm125Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_125_tokenizer_frequency() {
        let t = super::Xm125Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_125_tokenizer_delimiter() {
        let t = super::Xm125Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_125_tokenizer_whitespace() {
        let t = super::Xm125Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_125_tokenizer_empty() {
        let t = super::Xm125Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 125 ----

    #[test]
    fn xn_125_fenwick_prefix_sum() {
        let mut ft = super::Xn125Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_125_fenwick_range_sum() {
        let mut ft = super::Xn125Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_125_fenwick_point_query() {
        let mut ft = super::Xn125Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_125_fenwick_len() {
        let ft = super::Xn125Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_125_fenwick_multiple_updates() {
        let mut ft = super::Xn125Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_125_fenwick_single_element() {
        let mut ft = super::Xn125Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_125_fenwick_find_kth() {
        let mut ft = super::Xn125Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_125_fenwick_negative_delta() {
        let mut ft = super::Xn125Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 125 ----

    #[test]
    fn xn_125_avl_insert_get() {
        let mut m = super::Xn125AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_125_avl_remove() {
        let mut m = super::Xn125AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_125_avl_in_order() {
        let mut m = super::Xn125AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_125_avl_min_max() {
        let mut m = super::Xn125AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_125_avl_floor_ceiling() {
        let mut m = super::Xn125AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_125_avl_height_balanced() {
        let mut m = super::Xn125AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_125_avl_overwrite() {
        let mut m = super::Xn125AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_125_avl_empty() {
        let m: super::Xn125AVL<i32, i32> = super::Xn125AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
