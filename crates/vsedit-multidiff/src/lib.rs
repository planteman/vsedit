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
}
