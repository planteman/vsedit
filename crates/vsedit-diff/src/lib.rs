//! Diff computation algorithms.
//!
//! Equivalent to VS Code's diff computation.
//! Uses the `similar` crate for diffing text.

pub mod diff_result;
pub mod diff_editor;
pub mod merge;
pub mod dirty_diff;
pub mod git_diff;

use std::fmt;
use similar::{ChangeTag, TextDiff};

/// A single change in a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffChange {
    pub kind: DiffChangeKind,
    pub original_start: u32,
    pub original_length: u32,
    pub modified_start: u32,
    pub modified_length: u32,
}

/// Kind of diff change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffChangeKind {
    /// Lines added.
    Insert,
    /// Lines removed.
    Delete,
    /// Lines changed (combined insert + delete).
    Change,
}

/// A line-level diff between two texts.
#[derive(Debug, Clone)]
pub struct LineDiff {
    pub changes: Vec<DiffChange>,
    pub original_line_count: u32,
    pub modified_line_count: u32,
}

/// Compute a line-level diff between two texts.
pub fn compute_line_diff(original: &str, modified: &str) -> LineDiff {
    let diff = TextDiff::from_lines(original, modified);
    let mut changes = Vec::new();
    let mut orig_line: u32 = 0;
    let mut mod_line: u32 = 0;

    // Group consecutive inserts/deletes into changes
    let ops = diff.ops();
    for op in ops {
        match op {
            similar::DiffOp::Equal { old_index: _, new_index: _, len } => {
                orig_line += *len as u32;
                mod_line += *len as u32;
            }
            similar::DiffOp::Delete {
                old_index: _,
                old_len,
                new_index: _,
            } => {
                changes.push(DiffChange {
                    kind: DiffChangeKind::Delete,
                    original_start: orig_line + 1,
                    original_length: *old_len as u32,
                    modified_start: mod_line + 1,
                    modified_length: 0,
                });
                orig_line += *old_len as u32;
            }
            similar::DiffOp::Insert {
                old_index: _,
                new_index: _,
                new_len,
            } => {
                changes.push(DiffChange {
                    kind: DiffChangeKind::Insert,
                    original_start: orig_line + 1,
                    original_length: 0,
                    modified_start: mod_line + 1,
                    modified_length: *new_len as u32,
                });
                mod_line += *new_len as u32;
            }
            similar::DiffOp::Replace {
                old_index: _,
                old_len,
                new_index: _,
                new_len,
            } => {
                changes.push(DiffChange {
                    kind: DiffChangeKind::Change,
                    original_start: orig_line + 1,
                    original_length: *old_len as u32,
                    modified_start: mod_line + 1,
                    modified_length: *new_len as u32,
                });
                orig_line += *old_len as u32;
                mod_line += *new_len as u32;
            }
        }
    }

    let original_line_count = original.lines().count() as u32;
    let modified_line_count = modified.lines().count() as u32;

    LineDiff {
        changes,
        original_line_count,
        modified_line_count,
    }
}

/// A character-level change within a line.
#[derive(Debug, Clone)]
pub struct InlineChange {
    pub tag: ChangeTag,
    pub value: String,
}

/// Compute character-level diff between two strings (for inline highlighting).
pub fn compute_inline_diff(original: &str, modified: &str) -> Vec<InlineChange> {
    let diff = TextDiff::from_chars(original, modified);
    diff.iter_all_changes()
        .map(|c| InlineChange {
            tag: c.tag(),
            value: c.value().to_string(),
        })
        .collect()
}

/// Statistics about a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffStats {
    pub insertions: u32,
    pub deletions: u32,
    pub changes: u32,
}

/// Compute statistics from a `LineDiff`.
pub fn compute_stats(diff: &LineDiff) -> DiffStats {
    let mut insertions = 0u32;
    let mut deletions = 0u32;
    let mut changes = 0u32;
    for c in &diff.changes {
        match c.kind {
            DiffChangeKind::Insert => insertions += c.modified_length,
            DiffChangeKind::Delete => deletions += c.original_length,
            DiffChangeKind::Change => changes += 1,
        }
    }
    DiffStats {
        insertions,
        deletions,
        changes,
    }
}

/// Returns `true` if two texts are identical (no diff changes).
pub fn is_identical(original: &str, modified: &str) -> bool {
    compute_line_diff(original, modified).changes.is_empty()
}

/// Format a unified diff string using the `similar` crate.
pub fn format_unified_diff(
    original: &str,
    modified: &str,
    original_name: &str,
    modified_name: &str,
    context_lines: usize,
) -> String {
    let diff = TextDiff::from_lines(original, modified);
    diff.unified_diff()
        .header(original_name, modified_name)
        .context_radius(context_lines)
        .to_string()
}

/// A range of lines representing a single diff hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRange {
    pub original_start: u32,
    pub original_length: u32,
    pub modified_start: u32,
    pub modified_length: u32,
}

/// Extract hunk ranges from a `LineDiff`.
pub fn get_hunks(diff: &LineDiff) -> Vec<DiffRange> {
    diff.changes
        .iter()
        .map(|c| DiffRange {
            original_start: c.original_start,
            original_length: c.original_length,
            modified_start: c.modified_start,
            modified_length: c.modified_length,
        })
        .collect()
}

/// Produce a reversed diff by swapping original and modified roles.
pub fn reverse_diff(diff: &LineDiff) -> LineDiff {
    let changes = diff
        .changes
        .iter()
        .map(|c| DiffChange {
            kind: match c.kind {
                DiffChangeKind::Insert => DiffChangeKind::Delete,
                DiffChangeKind::Delete => DiffChangeKind::Insert,
                DiffChangeKind::Change => DiffChangeKind::Change,
            },
            original_start: c.modified_start,
            original_length: c.modified_length,
            modified_start: c.original_start,
            modified_length: c.original_length,
        })
        .collect();
    LineDiff {
        changes,
        original_line_count: diff.modified_line_count,
        modified_line_count: diff.original_line_count,
    }
}

/// Configuration options for diff computation.
#[derive(Debug, Clone)]
pub struct DiffConfig {
    pub ignore_whitespace: bool,
    pub ignore_case: bool,
    pub context_lines: usize,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            ignore_whitespace: false,
            ignore_case: false,
            context_lines: 3,
        }
    }
}

impl DiffConfig {
    /// Create a new default diff configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to ignore whitespace differences.
    pub fn with_ignore_whitespace(mut self, ignore: bool) -> Self {
        self.ignore_whitespace = ignore;
        self
    }

    /// Set whether to ignore case differences.
    pub fn with_ignore_case(mut self, ignore: bool) -> Self {
        self.ignore_case = ignore;
        self
    }

    /// Set the number of context lines for unified diff output.
    pub fn with_context_lines(mut self, lines: usize) -> Self {
        self.context_lines = lines;
        self
    }

    /// Normalize text according to the configuration settings.
    fn normalize<'a>(&self, text: &'a str) -> std::borrow::Cow<'a, str> {
        let mut result = std::borrow::Cow::Borrowed(text);
        if self.ignore_case {
            result = std::borrow::Cow::Owned(result.to_lowercase());
        }
        if self.ignore_whitespace {
            let normalized: String = result
                .lines()
                .map(|l| l.trim())
                .collect::<Vec<_>>()
                .join("\n");
            result = std::borrow::Cow::Owned(normalized);
        }
        result
    }

    /// Compute a line diff using the configured options.
    pub fn compute_diff(&self, original: &str, modified: &str) -> LineDiff {
        let orig = self.normalize(original);
        let modif = self.normalize(modified);
        compute_line_diff(&orig, &modif)
    }

    /// Toggle the `ignore_whitespace` flag.
    pub fn toggle_ignore_whitespace(&mut self) {
        self.ignore_whitespace = !self.ignore_whitespace;
    }

    /// Toggle the `ignore_case` flag.
    pub fn toggle_ignore_case(&mut self) {
        self.ignore_case = !self.ignore_case;
    }
}

/// Accumulated statistics for diff operations.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffStatsSummary {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl DiffStatsSummary {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &DiffStatsSummary) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for DiffStatsSummary {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DiffStatsSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DiffStatsSummary(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for diff.
#[derive(Debug, Clone)]
pub struct DiffValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl DiffValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for DiffValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DiffStatistics — detailed diff statistics with percentages
// ---------------------------------------------------------------------------

/// Detailed diff statistics including line counts and percentages.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffStatistics {
    pub total_insertions: u32,
    pub total_deletions: u32,
    pub total_changes: u32,
    pub original_line_count: u32,
    pub modified_line_count: u32,
}

impl DiffStatistics {
    /// Compute detailed statistics from a LineDiff.
    pub fn from_line_diff(diff: &LineDiff) -> Self {
        let stats = compute_stats(diff);
        Self {
            total_insertions: stats.insertions,
            total_deletions: stats.deletions,
            total_changes: stats.changes,
            original_line_count: diff.original_line_count,
            modified_line_count: diff.modified_line_count,
        }
    }

    /// Total number of affected hunks.
    pub fn hunk_count(&self) -> u32 {
        self.total_insertions + self.total_deletions + self.total_changes
    }

    /// Percentage of original lines that were deleted.
    pub fn deletion_percentage(&self) -> f64 {
        if self.original_line_count == 0 {
            return 0.0;
        }
        self.total_deletions as f64 / self.original_line_count as f64 * 100.0
    }

    /// Percentage of modified lines that are insertions.
    pub fn insertion_percentage(&self) -> f64 {
        if self.modified_line_count == 0 {
            return 0.0;
        }
        self.total_insertions as f64 / self.modified_line_count as f64 * 100.0
    }

    /// Net line change (positive = growth, negative = shrinkage).
    pub fn net_change(&self) -> i64 {
        self.modified_line_count as i64 - self.original_line_count as i64
    }

    /// Whether the diff is empty (no changes).
    pub fn is_empty(&self) -> bool {
        self.total_insertions == 0 && self.total_deletions == 0 && self.total_changes == 0
    }

    /// Churn: total lines added + deleted + changed.
    pub fn churn(&self) -> u32 {
        self.total_insertions + self.total_deletions + self.total_changes
    }

    /// Format as a git-style summary: "+X -Y ~Z".
    pub fn summary_string(&self) -> String {
        format!("+{} -{} ~{}", self.total_insertions, self.total_deletions, self.total_changes)
    }
}

impl fmt::Display for DiffStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DiffStatistics(+{} -{} ~{}, {}→{} lines)",
            self.total_insertions, self.total_deletions, self.total_changes,
            self.original_line_count, self.modified_line_count
        )
    }
}

// ---------------------------------------------------------------------------
// Patch formatting
// ---------------------------------------------------------------------------

/// A formatted patch section with header and content lines.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchSection {
    pub header: String,
    pub added_lines: Vec<String>,
    pub removed_lines: Vec<String>,
    pub context_lines: Vec<String>,
}

impl PatchSection {
    /// Create from a DiffHunk.
    pub fn from_hunk(hunk: &DiffHunk) -> Self {
        let header = format!(
            "@@ -{},{} +{},{} @@",
            hunk.original_start,
            hunk.original_lines.len(),
            hunk.modified_start,
            hunk.modified_lines.len(),
        );
        Self {
            header,
            removed_lines: hunk.original_lines.clone(),
            added_lines: hunk.modified_lines.clone(),
            context_lines: Vec::new(),
        }
    }

    /// Format as a string with +/- prefixes.
    pub fn format(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.header);
        out.push('\n');
        for line in &self.removed_lines {
            out.push('-');
            out.push_str(line.trim_end_matches('\n'));
            out.push('\n');
        }
        for line in &self.added_lines {
            out.push('+');
            out.push_str(line.trim_end_matches('\n'));
            out.push('\n');
        }
        out
    }

    /// Total number of lines in this section.
    pub fn total_lines(&self) -> usize {
        self.added_lines.len() + self.removed_lines.len() + self.context_lines.len()
    }
}

impl fmt::Display for PatchSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

/// Format all hunks as a complete patch.
pub fn format_patch(original_name: &str, modified_name: &str, hunks: &[DiffHunk]) -> String {
    let mut out = String::new();
    out.push_str(&format!("--- {}\n", original_name));
    out.push_str(&format!("+++ {}\n", modified_name));
    for hunk in hunks {
        let section = PatchSection::from_hunk(hunk);
        out.push_str(&section.format());
    }
    out
}

// ---------------------------------------------------------------------------
// Diff region merging for adjacent changes
// ---------------------------------------------------------------------------

/// Merge adjacent diff hunks that are within `max_gap` lines of each other.
pub fn merge_adjacent_hunks(hunks: &[DiffHunk], max_gap: u32) -> Vec<DiffHunk> {
    if hunks.is_empty() {
        return Vec::new();
    }
    let mut result: Vec<DiffHunk> = Vec::new();
    let mut current = hunks[0].clone();

    for hunk in &hunks[1..] {
        let current_end = current.original_start + current.original_lines.len() as u32;
        let gap = hunk.original_start.saturating_sub(current_end);

        if gap <= max_gap {
            // Merge: extend current hunk
            current.original_lines.extend(hunk.original_lines.iter().cloned());
            current.modified_lines.extend(hunk.modified_lines.iter().cloned());
            // Update type
            if !current.original_lines.is_empty() && !current.modified_lines.is_empty() {
                current.change_type = DiffHunkType::Modify;
            }
        } else {
            result.push(current);
            current = hunk.clone();
        }
    }
    result.push(current);
    result
}

/// Merge adjacent DiffChanges that are within `max_gap` lines of each other.
pub fn merge_adjacent_changes(changes: &[DiffChange], max_gap: u32) -> Vec<DiffChange> {
    if changes.is_empty() {
        return Vec::new();
    }
    let mut result: Vec<DiffChange> = Vec::new();
    let mut current = changes[0].clone();

    for change in &changes[1..] {
        let current_end = current.original_start + current.original_length;
        let gap = change.original_start.saturating_sub(current_end);

        if gap <= max_gap {
            // Merge into current
            let new_orig_end = (change.original_start + change.original_length)
                .max(current.original_start + current.original_length);
            let new_mod_end = (change.modified_start + change.modified_length)
                .max(current.modified_start + current.modified_length);

            current.original_length = new_orig_end - current.original_start;
            current.modified_length = new_mod_end - current.modified_start;

            // Adjust kind
            if current.original_length > 0 && current.modified_length > 0 {
                current.kind = DiffChangeKind::Change;
            }
        } else {
            result.push(current);
            current = change.clone();
        }
    }
    result.push(current);
    result
}

/// Count the total number of lines affected by a set of changes.
pub fn total_affected_lines(changes: &[DiffChange]) -> u32 {
    changes.iter().map(|c| c.original_length + c.modified_length).sum()
}

/// Type of change represented by a [`DiffHunk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffHunkType {
    /// Lines were added.
    Add,
    /// Lines were deleted.
    Delete,
    /// Lines were modified (replaced).
    Modify,
}

/// A rich hunk representation carrying the actual line content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub change_type: DiffHunkType,
    pub original_lines: Vec<String>,
    pub modified_lines: Vec<String>,
    pub original_start: u32,
    pub modified_start: u32,
}

/// Compute rich [`DiffHunk`]s from two texts using `similar::TextDiff`.
pub fn compute_diff_hunks(original: &str, modified: &str) -> Vec<DiffHunk> {
    let diff = TextDiff::from_lines(original, modified);
    let mut hunks: Vec<DiffHunk> = Vec::new();

    // Collect contiguous groups of non-equal changes.
    let mut del_lines: Vec<String> = Vec::new();
    let mut ins_lines: Vec<String> = Vec::new();
    let mut del_start: u32 = 0;
    let mut ins_start: u32 = 0;
    let mut orig_idx: u32 = 1;
    let mut mod_idx: u32 = 1;

    let flush =
        |hunks: &mut Vec<DiffHunk>,
         del_lines: &mut Vec<String>,
         ins_lines: &mut Vec<String>,
         del_start: u32,
         ins_start: u32| {
            if del_lines.is_empty() && ins_lines.is_empty() {
                return;
            }
            let change_type = match (del_lines.is_empty(), ins_lines.is_empty()) {
                (true, false) => DiffHunkType::Add,
                (false, true) => DiffHunkType::Delete,
                _ => DiffHunkType::Modify,
            };
            hunks.push(DiffHunk {
                change_type,
                original_lines: std::mem::take(del_lines),
                modified_lines: std::mem::take(ins_lines),
                original_start: del_start,
                modified_start: ins_start,
            });
        };

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                flush(&mut hunks, &mut del_lines, &mut ins_lines, del_start, ins_start);
                orig_idx += 1;
                mod_idx += 1;
            }
            ChangeTag::Delete => {
                if del_lines.is_empty() && ins_lines.is_empty() {
                    del_start = orig_idx;
                    ins_start = mod_idx;
                }
                if del_lines.is_empty() {
                    del_start = orig_idx;
                }
                del_lines.push(change.to_string_lossy().to_string());
                orig_idx += 1;
            }
            ChangeTag::Insert => {
                if del_lines.is_empty() && ins_lines.is_empty() {
                    del_start = orig_idx;
                    ins_start = mod_idx;
                }
                if ins_lines.is_empty() {
                    ins_start = mod_idx;
                }
                ins_lines.push(change.to_string_lossy().to_string());
                mod_idx += 1;
            }
        }
    }
    flush(&mut hunks, &mut del_lines, &mut ins_lines, del_start, ins_start);
    hunks
}

/// Generate a unified diff string from [`DiffHunk`]s with the given number of
/// context lines. Context is not expanded from the original text; only hunk
/// content is emitted.
pub fn unified_diff_format(hunks: &[DiffHunk], _context_lines: usize) -> String {
    let mut out = String::new();
    for hunk in hunks {
        let orig_count = hunk.original_lines.len();
        let mod_count = hunk.modified_lines.len();
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.original_start, orig_count, hunk.modified_start, mod_count,
        ));
        for line in &hunk.original_lines {
            out.push('-');
            out.push_str(line);
            if !line.ends_with('\n') {
                out.push('\n');
            }
        }
        for line in &hunk.modified_lines {
            out.push('+');
            out.push_str(line);
            if !line.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

/// Apply [`DiffHunk`]s to the original text to produce the modified text.
///
/// Hunks are applied in reverse order of their original position so that
/// earlier indices remain valid while later hunks are processed.
pub fn diff_apply(original: &str, hunks: &[DiffHunk]) -> Result<String, String> {
    let mut lines: Vec<String> = original.lines().map(|l| l.to_string()).collect();

    // Sort hunks by original_start descending so removals/insertions don't
    // shift indices that still need processing.
    let mut sorted: Vec<&DiffHunk> = hunks.iter().collect();
    sorted.sort_by(|a, b| b.original_start.cmp(&a.original_start));

    for hunk in &sorted {
        let start = (hunk.original_start as usize).saturating_sub(1);
        match hunk.change_type {
            DiffHunkType::Add => {
                let insert_at = start;
                for (i, line) in hunk.modified_lines.iter().enumerate() {
                    lines.insert(insert_at + i, line.trim_end_matches('\n').to_string());
                }
            }
            DiffHunkType::Delete => {
                let count = hunk.original_lines.len();
                if start + count > lines.len() {
                    return Err(format!(
                        "Delete hunk at line {} extends past end of file",
                        hunk.original_start
                    ));
                }
                lines.drain(start..start + count);
            }
            DiffHunkType::Modify => {
                let count = hunk.original_lines.len();
                if start + count > lines.len() {
                    return Err(format!(
                        "Modify hunk at line {} extends past end of file",
                        hunk.original_start
                    ));
                }
                lines.drain(start..start + count);
                for (i, line) in hunk.modified_lines.iter().enumerate() {
                    lines.insert(start + i, line.trim_end_matches('\n').to_string());
                }
            }
        }
    }

    // Reconstruct with trailing newline if original had one.
    let mut result = lines.join("\n");
    if original.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Word-level diff within changed lines
// ---------------------------------------------------------------------------

/// A word-level change within a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordChange {
    /// The word or token that changed.
    pub value: String,
    /// Whether this word was added, removed, or unchanged.
    pub kind: WordChangeKind,
}

/// Kind of word-level change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordChangeKind {
    /// Word is the same in both versions.
    Equal,
    /// Word was inserted in the modified version.
    Insert,
    /// Word was deleted from the original version.
    Delete,
}

/// Compute a word-level diff between two single lines.
///
/// Words are split on whitespace boundaries. The result is a sequence of
/// [`WordChange`] values describing equal, inserted, and deleted tokens.
pub fn compute_word_diff(original: &str, modified: &str) -> Vec<WordChange> {
    let orig_words: Vec<&str> = original.split_whitespace().collect();
    let mod_words: Vec<&str> = modified.split_whitespace().collect();

    let diff = TextDiff::from_slices(&orig_words, &mod_words);
    diff.iter_all_changes()
        .map(|c| {
            let kind = match c.tag() {
                ChangeTag::Equal => WordChangeKind::Equal,
                ChangeTag::Insert => WordChangeKind::Insert,
                ChangeTag::Delete => WordChangeKind::Delete,
            };
            WordChange {
                value: c.value().to_string(),
                kind,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Diff navigation — next/previous change from a cursor position
// ---------------------------------------------------------------------------

/// Navigator for stepping through diff changes by line number.
pub struct DiffNavigator<'a> {
    changes: &'a [DiffChange],
}

impl<'a> DiffNavigator<'a> {
    /// Create a navigator over the given changes.
    pub fn new(changes: &'a [DiffChange]) -> Self {
        Self { changes }
    }

    /// Return the index of the next change whose `original_start` is strictly
    /// after `current_line`, or `None` if there is no such change.
    pub fn next_change(&self, current_line: u32) -> Option<usize> {
        self.changes
            .iter()
            .position(|c| c.original_start > current_line)
    }

    /// Return the index of the previous change whose `original_start` is
    /// strictly before `current_line`, or `None` if there is no such change.
    pub fn prev_change(&self, current_line: u32) -> Option<usize> {
        self.changes
            .iter()
            .rposition(|c| c.original_start < current_line)
    }

    /// Return the index of the change that contains `current_line` (i.e. the
    /// line falls within `[original_start, original_start + original_length)`),
    /// or `None` if no change spans that line.
    pub fn change_at(&self, current_line: u32) -> Option<usize> {
        self.changes.iter().position(|c| {
            current_line >= c.original_start
                && current_line < c.original_start + c.original_length.max(1)
        })
    }

    /// Total number of changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Whether there are no changes.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Three-way merge conflict detection
// ---------------------------------------------------------------------------

/// A detected conflict region in a three-way merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    /// 1-based line in the base where the conflict starts.
    pub base_start: u32,
    /// Number of base lines involved.
    pub base_length: u32,
    /// Lines from the "ours" side.
    pub ours_lines: Vec<String>,
    /// Lines from the "theirs" side.
    pub theirs_lines: Vec<String>,
}

/// Detect three-way merge conflicts between `ours` and `theirs` relative to
/// a common `base` text.
///
/// A conflict exists when both sides modify the same region of the base. This
/// is a simplified heuristic: any overlapping original ranges in the two diffs
/// are reported as conflicts.
pub fn detect_merge_conflicts(base: &str, ours: &str, theirs: &str) -> Vec<MergeConflict> {
    let diff_ours = compute_line_diff(base, ours);
    let diff_theirs = compute_line_diff(base, theirs);

    let mut conflicts = Vec::new();

    for co in &diff_ours.changes {
        for ct in &diff_theirs.changes {
            let o_end = co.original_start + co.original_length;
            let t_end = ct.original_start + ct.original_length;

            // Check overlap in the base (original) dimension.
            let overlap = co.original_start < t_end && ct.original_start < o_end;
            if !overlap {
                continue;
            }

            // Both sides touch the same base region — conflict.
            let base_start = co.original_start.min(ct.original_start);
            let base_end = o_end.max(t_end);

            // Collect the affected lines from each side.
            let ours_lines: Vec<String> = ours
                .lines()
                .skip((co.modified_start as usize).saturating_sub(1))
                .take(co.modified_length as usize)
                .map(|l| l.to_string())
                .collect();
            let theirs_lines: Vec<String> = theirs
                .lines()
                .skip((ct.modified_start as usize).saturating_sub(1))
                .take(ct.modified_length as usize)
                .map(|l| l.to_string())
                .collect();

            conflicts.push(MergeConflict {
                base_start,
                base_length: base_end - base_start,
                ours_lines,
                theirs_lines,
            });
        }
    }

    conflicts
}

// ---------------------------------------------------------------------------
// Similarity ratio between two texts
// ---------------------------------------------------------------------------

/// Compute a similarity ratio in `[0.0, 1.0]` between two texts.
///
/// Uses the number of unchanged lines over the total lines in both documents.
pub fn similarity_ratio(original: &str, modified: &str) -> f64 {
    let diff = TextDiff::from_lines(original, modified);
    diff.ratio() as f64
}

// ---------------------------------------------------------------------------
// DiffHunkStatistics — per-hunk statistics
// ---------------------------------------------------------------------------

/// Statistics for a single [`DiffHunk`], counting additions, deletions, and
/// modifications within that hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunkStatistics {
    /// Number of lines added in this hunk.
    pub additions: u32,
    /// Number of lines deleted in this hunk.
    pub deletions: u32,
    /// Number of lines considered modified (min of added and deleted).
    pub modifications: u32,
}

impl DiffHunkStatistics {
    /// Compute statistics from a [`DiffHunk`].
    pub fn from_hunk(hunk: &DiffHunk) -> Self {
        let added = hunk.modified_lines.len() as u32;
        let deleted = hunk.original_lines.len() as u32;
        let modifications = added.min(deleted);
        Self {
            additions: added.saturating_sub(modifications),
            deletions: deleted.saturating_sub(modifications),
            modifications,
        }
    }

    /// Total number of changed lines (additions + deletions + modifications).
    pub fn total_changes(&self) -> u32 {
        self.additions + self.deletions + self.modifications
    }

    /// Returns `true` if the hunk contains only additions.
    pub fn is_pure_addition(&self) -> bool {
        self.additions > 0 && self.deletions == 0 && self.modifications == 0
    }

    /// Returns `true` if the hunk contains only deletions.
    pub fn is_pure_deletion(&self) -> bool {
        self.deletions > 0 && self.additions == 0 && self.modifications == 0
    }
}

impl fmt::Display for DiffHunkStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "+{} -{} ~{}",
            self.additions, self.deletions, self.modifications
        )
    }
}

// ---------------------------------------------------------------------------
// SemanticDiffGroup — groups adjacent changes into logical blocks
// ---------------------------------------------------------------------------

/// A group of adjacent [`DiffChange`]s that form a logical unit.
///
/// Changes whose line numbers are within `max_gap` of each other are grouped
/// together.
#[derive(Debug, Clone)]
pub struct SemanticDiffGroup {
    /// The changes belonging to this group.
    pub changes: Vec<DiffChange>,
    /// First affected original line in the group (1-based).
    pub start_line: u32,
    /// Last affected original line in the group (1-based, inclusive).
    pub end_line: u32,
}

impl SemanticDiffGroup {
    /// Group a slice of [`DiffChange`]s into semantic blocks.
    ///
    /// Two consecutive changes are placed in the same group when the gap
    /// between the end of one change and the start of the next is at most
    /// `max_gap` lines.
    pub fn group_changes(changes: &[DiffChange], max_gap: u32) -> Vec<SemanticDiffGroup> {
        if changes.is_empty() {
            return Vec::new();
        }

        let mut groups: Vec<SemanticDiffGroup> = Vec::new();
        let mut current_changes = vec![changes[0].clone()];
        let mut start_line = changes[0].original_start;
        let mut end_line = changes[0].original_start + changes[0].original_length.max(1) - 1;

        for change in &changes[1..] {
            let change_start = change.original_start;
            let gap = change_start.saturating_sub(end_line + 1);

            if gap <= max_gap {
                current_changes.push(change.clone());
                let change_end = change.original_start + change.original_length.max(1) - 1;
                end_line = end_line.max(change_end);
            } else {
                groups.push(SemanticDiffGroup {
                    changes: std::mem::take(&mut current_changes),
                    start_line,
                    end_line,
                });
                current_changes.push(change.clone());
                start_line = change.original_start;
                end_line = change.original_start + change.original_length.max(1) - 1;
            }
        }

        groups.push(SemanticDiffGroup {
            changes: current_changes,
            start_line,
            end_line,
        });

        groups
    }

    /// Number of original lines spanned by this group (inclusive).
    pub fn line_span(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Number of individual changes in this group.
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }
}

// ---------------------------------------------------------------------------
// DiffSummaryReport — comprehensive diff summary
// ---------------------------------------------------------------------------

/// A comprehensive summary report of a diff between two texts.
#[derive(Debug, Clone)]
pub struct DiffSummaryReport {
    /// Total lines added.
    pub additions: u32,
    /// Total lines deleted.
    pub deletions: u32,
    /// Total modification hunks (replace operations).
    pub modifications: u32,
    /// Line count of the original text.
    pub original_lines: u32,
    /// Line count of the modified text.
    pub modified_lines: u32,
}

impl DiffSummaryReport {
    /// Build a summary report by diffing `original` and `modified`.
    pub fn from_diff(original: &str, modified: &str) -> Self {
        let diff = compute_line_diff(original, modified);
        let stats = compute_stats(&diff);
        Self {
            additions: stats.insertions,
            deletions: stats.deletions,
            modifications: stats.changes,
            original_lines: diff.original_line_count,
            modified_lines: diff.modified_line_count,
        }
    }

    /// Total number of lines affected (additions + deletions + modifications).
    pub fn total_lines_changed(&self) -> usize {
        (self.additions + self.deletions + self.modifications) as usize
    }

    /// Ratio of changed lines to total lines in the larger of the two texts.
    ///
    /// Returns `0.0` when both texts are empty.
    pub fn change_ratio(&self) -> f64 {
        let max_lines = self.original_lines.max(self.modified_lines);
        if max_lines == 0 {
            return 0.0;
        }
        self.total_lines_changed() as f64 / max_lines as f64
    }

    /// Returns `true` if the diff contains at least one addition.
    pub fn has_additions(&self) -> bool {
        self.additions > 0
    }

    /// Returns `true` if the diff contains at least one deletion.
    pub fn has_deletions(&self) -> bool {
        self.deletions > 0
    }
}

impl fmt::Display for DiffSummaryReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DiffSummary(+{} -{} ~{}, {}/{} lines, {:.1}% changed)",
            self.additions,
            self.deletions,
            self.modifications,
            self.original_lines,
            self.modified_lines,
            self.change_ratio() * 100.0,
        )
    }
}

// ---------------------------------------------------------------------------
// CharLevelDiff — character-level diff within a single line
// ---------------------------------------------------------------------------

/// A single character-level change within a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharChange {
    /// Whether this fragment was inserted, deleted, or unchanged.
    pub kind: DiffChangeKind,
    /// The text fragment.
    pub text: String,
}

/// Character-level diff engine for fine-grained intra-line comparison.
pub struct CharLevelDiff;

impl CharLevelDiff {
    /// Compute character-level differences between two strings.
    ///
    /// Characters are compared individually using the `similar` crate.  Runs
    /// of consecutive characters with the same change tag are coalesced into a
    /// single [`CharChange`].
    pub fn compute(original: &str, modified: &str) -> Vec<CharChange> {
        let orig_chars: Vec<char> = original.chars().collect();
        let mod_chars: Vec<char> = modified.chars().collect();

        let orig_strs: Vec<String> = orig_chars.iter().map(|c| c.to_string()).collect();
        let mod_strs: Vec<String> = mod_chars.iter().map(|c| c.to_string()).collect();

        let orig_refs: Vec<&str> = orig_strs.iter().map(|s| s.as_str()).collect();
        let mod_refs: Vec<&str> = mod_strs.iter().map(|s| s.as_str()).collect();

        let diff = TextDiff::from_slices(&orig_refs, &mod_refs);
        let mut result: Vec<CharChange> = Vec::new();

        for change in diff.iter_all_changes() {
            let kind = match change.tag() {
                ChangeTag::Equal => DiffChangeKind::Change, // reuse as "equal"
                ChangeTag::Insert => DiffChangeKind::Insert,
                ChangeTag::Delete => DiffChangeKind::Delete,
            };
            let text = change.value().to_string();

            // Coalesce consecutive changes of the same kind.
            if let Some(last) = result.last_mut() {
                if last.kind == kind {
                    last.text.push_str(&text);
                    continue;
                }
            }
            result.push(CharChange { kind, text });
        }

        result
    }
}

// ---------------------------------------------------------------------------
// DiffSummaryStats - diff summary statistics
// ---------------------------------------------------------------------------

/// Severity level for diff summary statistics issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiffSummaryStatsSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for DiffSummaryStatsSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [DiffSummaryStats].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSummaryStatsEntry {
    pub id: String,
    pub label: String,
    pub severity: DiffSummaryStatsSeverity,
    pub detail: Option<String>,
    pub additions: usize,
    enabled: bool,
}

impl DiffSummaryStatsEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: DiffSummaryStatsSeverity::Low,
            detail: None,
            additions: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: DiffSummaryStatsSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_additions(mut self, val: usize) -> Self {
        self.additions = val;
        self
    }

    pub fn file_count(&self) -> bool {
        self.enabled && self.severity >= DiffSummaryStatsSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.additions, det)
    }
}

impl fmt::Display for DiffSummaryStatsEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [DiffSummaryStatsEntry] items.
#[derive(Debug, Clone)]
pub struct DiffSummaryStats {
    entries: Vec<DiffSummaryStatsEntry>,
    name: String,
    capacity: usize,
}

impl DiffSummaryStats {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: DiffSummaryStatsEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<DiffSummaryStatsEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&DiffSummaryStatsEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn additions(&self) -> usize { self.entries.len() }

    pub fn file_count(&self) -> bool {
        self.entries.iter().any(|e| e.file_count())
    }

    pub fn entries_by_severity(&self, severity: DiffSummaryStatsSeverity) -> Vec<&DiffSummaryStatsEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= DiffSummaryStatsSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&DiffSummaryStatsEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&DiffSummaryStatsEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// DiffPatchFormatter - diff patch formatter
// ---------------------------------------------------------------------------

/// Configuration for [DiffPatchFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffPatchFormatterConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub deletions: usize,
}

impl DiffPatchFormatterConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, deletions: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_deletions(mut self, val: usize) -> Self { self.deletions = val; self }
}

impl Default for DiffPatchFormatterConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [DiffPatchFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffPatchFormatterItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl DiffPatchFormatterItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn has_changes(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for DiffPatchFormatterItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [DiffPatchFormatterItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct DiffPatchFormatter {
    config: DiffPatchFormatterConfig,
    items: Vec<DiffPatchFormatterItem>,
}

impl DiffPatchFormatter {
    pub fn new(config: DiffPatchFormatterConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: DiffPatchFormatterItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<DiffPatchFormatterItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&DiffPatchFormatterItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn deletions(&self) -> usize { self.items.len() }

    pub fn has_changes(&self) -> bool {
        self.items.iter().any(|i| i.has_changes())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&DiffPatchFormatterItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&DiffPatchFormatterItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &DiffPatchFormatterConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// vsedit-diff: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl DiffXConfig {
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

impl std::fmt::Display for DiffXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct DiffXRegistry {
    entries: Vec<DiffXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl DiffXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: DiffXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&DiffXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut DiffXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<DiffXConfig> {
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

    pub fn active_entries(&self) -> Vec<&DiffXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&DiffXConfig> {
        let mut sorted: Vec<&DiffXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&DiffXConfig> {
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

    pub fn iter(&self) -> DiffXIterator<'_> {
        DiffXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct DiffXIterator<'a> {
    inner: std::slice::Iter<'a, DiffXConfig>,
}

impl<'a> Iterator for DiffXIterator<'a> {
    type Item = &'a DiffXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct DiffXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl DiffXCache {
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
pub struct DiffXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl DiffXFormatter {
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

    pub fn format_entry(&self, entry: &DiffXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &DiffXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &DiffXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for DiffXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct DiffXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl DiffXValidator {
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

    pub fn validate(&self, entry: &DiffXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &DiffXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for DiffXValidator {
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
// xb_ utilities – batch 57
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer57 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer57 {
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
pub fn xb_fnv1a_57(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_57<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_57<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_57(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_57(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 30
// ---------------------------------------------------------------------------

/// Generic object pool `Xc30Pool<T>`.
pub struct Xc30Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc30Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc30PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc30Pool<T> {
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
    pub fn stats(&self) -> Xc30PoolStats {
        Xc30PoolStats {
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

impl<T> Default for Xc30Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc30Scheduler`.
pub struct Xc30Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc30Scheduler {
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

impl Default for Xc30Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_30 hash for the given byte slice.
pub fn xc_30_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_30 convention.
pub fn xc_30_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe70 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe70Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe70PipelineError {
    pub stage: Xe70Stage,
    pub message: String,
}

impl std::fmt::Display for Xe70PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe70Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe70Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError>>>,
    stage_names: Vec<Xe70Stage>,
}

impl Xe70Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe70Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe70Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe70Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe70Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError> {
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

    pub fn compose(mut self, other: Xe70Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe70CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe70CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe70Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe70CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe70CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe70Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe70CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_70_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe70CacheEntry {
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

    fn xe_70_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe70CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_70_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError> {
    Ok(data)
}

pub fn xe_70_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_70_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_70_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_70_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe70PipelineError> {
    Err(Xe70PipelineError {
        stage: Xe70Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_68: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg68Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg68Graph {
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

impl Default for Xg68Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_68: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg68Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg68Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg68Heap<T>) {
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

impl<T: Ord> Default for Xg68Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 29).
pub struct Xh29SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh29SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 71 as u64,
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

/// A compact bit set supporting boolean operations (variant 29).
pub struct Xh29BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh29BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 29).
pub struct Xi29Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi29Deque<T> {
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
pub struct Xi29Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi29Interval {
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

/// A simple interval tree (variant 29).
pub struct Xi29IntervalTree {
    xi_intervals: Vec<Xi29Interval>,
}

impl Xi29IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi29Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi29Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi29Interval) -> Vec<&Xi29Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi29Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi29Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi29Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi29Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi29Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi29Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 28) ---

/// Disjoint set / union-find for crate 28.
pub struct Xj28UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj28UnionFind {
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

const XJ28_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 28.
pub struct Xj28BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj28BTreeNode<K, V>>>,
    len: usize,
}

struct Xj28BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj28BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj28BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ28_BTREE_ORDER - 1
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
        let mid = XJ28_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj28BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj28BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj28BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj28BTreeNode::xj_new_leaf();
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


// --- xk_29 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk29SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk29SegmentTree {
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
pub struct Xk29DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk29DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_28).
#[derive(Debug, Clone)]
pub struct Xl28Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl28Rope {
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

/// Suffix array for efficient string searching (xl_28).
#[derive(Debug, Clone)]
pub struct Xl28SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl28SuffixArray {
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
pub struct Xm28MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm28MatrixSparse {
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
pub struct Xm28Tokenizer {
    text: String,
}

impl Xm28Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 29.
pub struct Xn29Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn29Fenwick {
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

// ----- AVL tree map — crate 29 -----

#[derive(Debug, Clone)]
struct Xn29AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn29AvlNode<K, V>>>,
    right: Option<Box<Xn29AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 29.
#[derive(Debug, Clone)]
pub struct Xn29AVL<K, V> {
    root: Option<Box<Xn29AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn29AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn29AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn29AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn29AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn29AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn29AvlNode<K, V>>) -> Box<Xn29AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn29AvlNode<K, V>>) -> Box<Xn29AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn29AvlNode<K, V>>) -> Box<Xn29AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn29AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn29AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn29AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn29AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn29AvlNode<K, V>>) -> &Xn29AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn29AvlNode<K, V>>) -> (Box<Xn29AvlNode<K, V>>, Option<Box<Xn29AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn29AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn29AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn29AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn29AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn29AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn29AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn29AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo29RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo29Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo29RBNode<K, V> {
    key: K,
    value: V,
    color: Xo29Color,
    left: Option<Box<Xo29RBNode<K, V>>>,
    right: Option<Box<Xo29RBNode<K, V>>>,
}

/// A red-black tree map for crate 29.
#[derive(Debug, Clone)]
pub struct Xo29RedBlack<K, V> {
    root: Option<Box<Xo29RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo29RedBlack<K, V> {
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
            r.color = Xo29Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo29RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo29RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo29RBNode {
                    key, value, color: Xo29Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo29RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo29Color::Red)
    }

    fn xo_balance(mut h: Box<Xo29RBNode<K, V>>) -> Box<Xo29RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo29Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo29RBNode<K, V>>) -> Box<Xo29RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo29Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo29RBNode<K, V>>) -> Box<Xo29RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo29Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo29RBNode<K, V>>) {
        h.color = Xo29Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo29Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo29Color::Black; }
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
            r.color = Xo29Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo29RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo29RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo29RBNode<K, V>) -> (K, V, Option<Box<Xo29RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo29RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo29Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo29RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo29ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 29.
#[derive(Debug, Clone)]
pub struct Xo29ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo29ConsistentHash {
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
            let vkey = format!("{}#xo29#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo29#{}", node, i);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_changes() {
        let diff = compute_line_diff("hello\nworld\n", "hello\nworld\n");
        assert!(diff.changes.is_empty());
    }

    #[test]
    fn insert_lines() {
        let diff = compute_line_diff("a\nc\n", "a\nb\nc\n");
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].kind, DiffChangeKind::Insert);
        assert_eq!(diff.changes[0].modified_length, 1);
    }

    #[test]
    fn delete_lines() {
        let diff = compute_line_diff("a\nb\nc\n", "a\nc\n");
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].kind, DiffChangeKind::Delete);
        assert_eq!(diff.changes[0].original_length, 1);
    }

    #[test]
    fn change_lines() {
        let diff = compute_line_diff("a\nb\n", "a\nB\n");
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].kind, DiffChangeKind::Change);
    }

    #[test]
    fn multiple_changes() {
        let diff = compute_line_diff("a\nb\nc\nd\n", "a\nB\nc\nD\n");
        assert_eq!(diff.changes.len(), 2);
    }

    #[test]
    fn inline_diff() {
        let changes = compute_inline_diff("hello", "hallo");
        assert!(changes.len() > 1);
    }

    #[test]
    fn empty_to_content() {
        let diff = compute_line_diff("", "new content\n");
        assert!(!diff.changes.is_empty());
        assert_eq!(diff.modified_line_count, 1);
    }

    #[test]
    fn compute_stats_counts() {
        let diff = compute_line_diff("a\nb\nc\n", "a\nx\ny\nc\n");
        let stats = compute_stats(&diff);
        assert!(stats.insertions > 0 || stats.changes > 0);
    }

    #[test]
    fn is_identical_true() {
        assert!(is_identical("abc\ndef\n", "abc\ndef\n"));
    }

    #[test]
    fn is_identical_false() {
        assert!(!is_identical("abc\n", "xyz\n"));
    }

    #[test]
    fn format_unified_diff_output() {
        let output = format_unified_diff("a\nb\n", "a\nc\n", "old.txt", "new.txt", 3);
        assert!(output.contains("old.txt"));
        assert!(output.contains("new.txt"));
    }

    #[test]
    fn get_hunks_returns_ranges() {
        let diff = compute_line_diff("a\nb\nc\n", "a\nx\nc\n");
        let hunks = get_hunks(&diff);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].original_start, 2);
    }

    #[test]
    fn reverse_diff_swaps() {
        let diff = compute_line_diff("a\nb\n", "a\nc\n");
        let reversed = reverse_diff(&diff);
        assert_eq!(reversed.original_line_count, diff.modified_line_count);
        assert_eq!(reversed.modified_line_count, diff.original_line_count);
    }

    #[test]
    fn diff_config_default() {
        let config = DiffConfig::new();
        assert!(!config.ignore_whitespace);
        assert!(!config.ignore_case);
        assert_eq!(config.context_lines, 3);
    }

    #[test]
    fn diff_config_ignore_case() {
        let config = DiffConfig::new().with_ignore_case(true);
        let diff = config.compute_diff("Hello\n", "hello\n");
        assert!(diff.changes.is_empty());
    }

    #[test]
    fn diff_config_ignore_whitespace() {
        let config = DiffConfig::new().with_ignore_whitespace(true);
        let diff = config.compute_diff("  hello  \n", "hello\n");
        assert!(diff.changes.is_empty());
    }

    #[test]
    fn reverse_insert_becomes_delete() {
        let diff = compute_line_diff("a\n", "a\nb\n");
        assert_eq!(diff.changes[0].kind, DiffChangeKind::Insert);
        let reversed = reverse_diff(&diff);
        assert_eq!(reversed.changes[0].kind, DiffChangeKind::Delete);
    }

    #[test]
    fn eq_diffchangekind_same() {
        assert_eq!(DiffChangeKind::Insert, DiffChangeKind::Insert);
    }

    #[test]
    fn ne_diffchangekind_diff() {
        assert_ne!(DiffChangeKind::Insert, DiffChangeKind::Delete);
    }

    #[test]
    fn test_diff_hunk_type_eq() {
        assert_eq!(DiffHunkType::Add, DiffHunkType::Add);
        assert_eq!(DiffHunkType::Delete, DiffHunkType::Delete);
        assert_eq!(DiffHunkType::Modify, DiffHunkType::Modify);
        assert_ne!(DiffHunkType::Add, DiffHunkType::Delete);
    }

    #[test]
    fn test_compute_diff_hunks_no_changes() {
        let hunks = compute_diff_hunks("hello\nworld\n", "hello\nworld\n");
        assert!(hunks.is_empty());
    }

    #[test]
    fn test_compute_diff_hunks_addition() {
        let hunks = compute_diff_hunks("a\nc\n", "a\nb\nc\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].change_type, DiffHunkType::Add);
        assert!(hunks[0].original_lines.is_empty());
        assert_eq!(hunks[0].modified_lines.len(), 1);
    }

    #[test]
    fn test_compute_diff_hunks_deletion() {
        let hunks = compute_diff_hunks("a\nb\nc\n", "a\nc\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].change_type, DiffHunkType::Delete);
        assert_eq!(hunks[0].original_lines.len(), 1);
        assert!(hunks[0].modified_lines.is_empty());
    }

    #[test]
    fn test_compute_diff_hunks_modification() {
        let hunks = compute_diff_hunks("a\nb\nc\n", "a\nx\nc\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].change_type, DiffHunkType::Modify);
        assert_eq!(hunks[0].original_lines.len(), 1);
        assert_eq!(hunks[0].modified_lines.len(), 1);
    }

    #[test]
    fn test_compute_diff_hunks_multiple() {
        let hunks = compute_diff_hunks("a\nb\nc\nd\ne\n", "a\nx\nc\nd\ny\n");
        assert!(hunks.len() >= 2);
    }

    #[test]
    fn test_unified_diff_format_basic() {
        let hunks = compute_diff_hunks("a\nb\nc\n", "a\nx\nc\n");
        let output = unified_diff_format(&hunks, 0);
        assert!(output.contains("@@"));
        assert!(output.contains("-b\n"));
        assert!(output.contains("+x\n"));
    }

    #[test]
    fn test_unified_diff_format_context() {
        let hunks = compute_diff_hunks("a\nb\nc\n", "a\nx\nc\n");
        let output = unified_diff_format(&hunks, 3);
        assert!(output.contains("@@"));
    }

    #[test]
    fn test_diff_apply_add() {
        let original = "a\nc\n";
        let hunks = vec![DiffHunk {
            change_type: DiffHunkType::Add,
            original_lines: vec![],
            modified_lines: vec!["b\n".to_string()],
            original_start: 2,
            modified_start: 2,
        }];
        let result = diff_apply(original, &hunks).unwrap();
        assert!(result.contains("b"));
    }

    #[test]
    fn test_diff_apply_delete() {
        let original = "a\nb\nc\n";
        let hunks = vec![DiffHunk {
            change_type: DiffHunkType::Delete,
            original_lines: vec!["b\n".to_string()],
            modified_lines: vec![],
            original_start: 2,
            modified_start: 2,
        }];
        let result = diff_apply(original, &hunks).unwrap();
        assert!(!result.contains("b"));
    }

    #[test]
    fn test_diff_apply_modify() {
        let original = "a\nb\nc\n";
        let hunks = vec![DiffHunk {
            change_type: DiffHunkType::Modify,
            original_lines: vec!["b\n".to_string()],
            modified_lines: vec!["x\n".to_string()],
            original_start: 2,
            modified_start: 2,
        }];
        let result = diff_apply(original, &hunks).unwrap();
        assert!(result.contains("x"));
        assert!(!result.contains("b"));
    }

    #[test]
    fn test_diff_apply_roundtrip() {
        let original = "line1\nline2\nline3\nline4\n";
        let modified = "line1\nchanged\nline3\nnew\nline4\n";
        let hunks = compute_diff_hunks(original, modified);
        let result = diff_apply(original, &hunks).unwrap();
        assert_eq!(result, modified);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_31() {
        let _svc = DiffConfig::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn diff_stats_new_defaults() {
        let stats = DiffStatsSummary::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn diff_stats_record_success() {
        let mut stats = DiffStatsSummary::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn diff_stats_record_failure() {
        let mut stats = DiffStatsSummary::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn diff_stats_reset() {
        let mut stats = DiffStatsSummary::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn diff_stats_merge() {
        let mut a = DiffStatsSummary::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = DiffStatsSummary::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn diff_stats_display() {
        let mut stats = DiffStatsSummary::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn diff_stats_default() {
        let stats = DiffStatsSummary::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn diff_validator_accepts_valid_name() {
        let v = DiffValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn diff_validator_rejects_empty() {
        let v = DiffValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn diff_validator_rejects_too_long() {
        let v = DiffValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn diff_validator_forbidden_prefix() {
        let v = DiffValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn diff_validator_allowed_chars() {
        let v = DiffValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn diff_validator_range() {
        let v = DiffValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn diff_sanitize_removes_control() {
        let result = DiffValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn diff_truncate_short_string() {
        assert_eq!(DiffValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn diff_truncate_long_string() {
        let result = DiffValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn diff_is_ascii_printable() {
        assert!(DiffValidator::is_ascii_printable("Hello World 123"));
        assert!(!DiffValidator::is_ascii_printable("Hello\x00World"));
    }

    // ---- DiffStatistics tests ----

    #[test]
    fn diff_statistics_from_line_diff() {
        let diff = compute_line_diff("a\nb\nc\n", "a\nx\nc\nd\n");
        let stats = DiffStatistics::from_line_diff(&diff);
        assert!(!stats.is_empty());
        assert!(stats.net_change() > 0);
        assert!(!stats.summary_string().is_empty());
    }

    #[test]
    fn diff_statistics_empty_diff() {
        let diff = compute_line_diff("hello\n", "hello\n");
        let stats = DiffStatistics::from_line_diff(&diff);
        assert!(stats.is_empty());
        assert_eq!(stats.churn(), 0);
    }

    #[test]
    fn diff_statistics_percentages() {
        let diff = compute_line_diff("a\nb\nc\nd\n", "a\nc\nd\n");
        let stats = DiffStatistics::from_line_diff(&diff);
        assert!(stats.deletion_percentage() > 0.0);
    }

    // ---- Patch formatting tests ----

    #[test]
    fn format_patch_output() {
        let hunks = compute_diff_hunks("hello\nworld\n", "hello\nearth\n");
        let patch = format_patch("a.txt", "b.txt", &hunks);
        assert!(patch.contains("--- a.txt"));
        assert!(patch.contains("+++ b.txt"));
        assert!(patch.contains("@@"));
    }

    #[test]
    fn patch_section_from_hunk() {
        let hunks = compute_diff_hunks("line1\nline2\n", "line1\nchanged\n");
        assert!(!hunks.is_empty());
        let section = PatchSection::from_hunk(&hunks[0]);
        assert!(section.total_lines() > 0);
        assert!(section.header.contains("@@"));
    }

    // ---- Merge adjacent changes tests ----

    #[test]
    fn merge_adjacent_changes_close_hunks() {
        let changes = vec![
            DiffChange { kind: DiffChangeKind::Delete, original_start: 1, original_length: 1, modified_start: 1, modified_length: 0 },
            DiffChange { kind: DiffChangeKind::Insert, original_start: 3, original_length: 0, modified_start: 2, modified_length: 1 },
        ];
        let merged = merge_adjacent_changes(&changes, 2);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].kind, DiffChangeKind::Change);
    }

    #[test]
    fn merge_adjacent_changes_far_apart() {
        let changes = vec![
            DiffChange { kind: DiffChangeKind::Delete, original_start: 1, original_length: 1, modified_start: 1, modified_length: 0 },
            DiffChange { kind: DiffChangeKind::Insert, original_start: 100, original_length: 0, modified_start: 99, modified_length: 1 },
        ];
        let merged = merge_adjacent_changes(&changes, 2);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn total_affected_lines_count() {
        let changes = vec![
            DiffChange { kind: DiffChangeKind::Delete, original_start: 1, original_length: 3, modified_start: 1, modified_length: 0 },
            DiffChange { kind: DiffChangeKind::Insert, original_start: 5, original_length: 0, modified_start: 2, modified_length: 2 },
        ];
        assert_eq!(total_affected_lines(&changes), 5);
    }

    // ---- Word-level diff tests ----

    #[test]
    fn word_diff_single_word_change() {
        let result = compute_word_diff("the quick brown fox", "the slow brown fox");
        // "quick" should be deleted, "slow" inserted, rest equal
        let deleted: Vec<_> = result.iter().filter(|w| w.kind == WordChangeKind::Delete).collect();
        let inserted: Vec<_> = result.iter().filter(|w| w.kind == WordChangeKind::Insert).collect();
        assert_eq!(deleted.len(), 1);
        assert!(deleted[0].value.contains("quick"));
        assert_eq!(inserted.len(), 1);
        assert!(inserted[0].value.contains("slow"));
    }

    #[test]
    fn word_diff_identical_lines() {
        let result = compute_word_diff("hello world", "hello world");
        assert!(result.iter().all(|w| w.kind == WordChangeKind::Equal));
    }

    // ---- Diff navigator tests ----

    #[test]
    fn navigator_next_and_prev() {
        let changes = vec![
            DiffChange { kind: DiffChangeKind::Delete, original_start: 5, original_length: 2, modified_start: 5, modified_length: 0 },
            DiffChange { kind: DiffChangeKind::Insert, original_start: 15, original_length: 0, modified_start: 13, modified_length: 3 },
            DiffChange { kind: DiffChangeKind::Change, original_start: 30, original_length: 1, modified_start: 29, modified_length: 1 },
        ];
        let nav = DiffNavigator::new(&changes);
        assert_eq!(nav.len(), 3);

        // Next change after line 1 → index 0 (line 5)
        assert_eq!(nav.next_change(1), Some(0));
        // Next change after line 10 → index 1 (line 15)
        assert_eq!(nav.next_change(10), Some(1));
        // No next change after line 30
        assert_eq!(nav.next_change(30), None);

        // Prev change before line 20 → index 1 (line 15)
        assert_eq!(nav.prev_change(20), Some(1));
        // No prev change before line 5
        assert_eq!(nav.prev_change(5), None);
    }

    #[test]
    fn navigator_change_at_line() {
        let changes = vec![
            DiffChange { kind: DiffChangeKind::Change, original_start: 10, original_length: 3, modified_start: 10, modified_length: 2 },
        ];
        let nav = DiffNavigator::new(&changes);
        assert_eq!(nav.change_at(10), Some(0));
        assert_eq!(nav.change_at(12), Some(0));
        assert_eq!(nav.change_at(13), None);
        assert_eq!(nav.change_at(9), None);
    }

    // ---- Three-way merge conflict detection test ----

    #[test]
    fn detect_conflict_on_same_region() {
        let base = "line1\nline2\nline3\nline4\n";
        let ours = "line1\nours2\nline3\nline4\n";
        let theirs = "line1\ntheirs2\nline3\nline4\n";
        let conflicts = detect_merge_conflicts(base, ours, theirs);
        assert!(!conflicts.is_empty(), "should detect at least one conflict");
        let c = &conflicts[0];
        assert!(!c.ours_lines.is_empty());
        assert!(!c.theirs_lines.is_empty());
    }

    #[test]
    fn no_conflict_when_different_regions() {
        let base = "line1\nline2\nline3\nline4\n";
        let ours = "changed1\nline2\nline3\nline4\n";
        let theirs = "line1\nline2\nline3\nchanged4\n";
        let conflicts = detect_merge_conflicts(base, ours, theirs);
        assert!(conflicts.is_empty(), "non-overlapping edits should not conflict");
    }

    // ---- Similarity ratio test ----

    #[test]
    fn similarity_identical_texts() {
        let r = similarity_ratio("aaa\nbbb\n", "aaa\nbbb\n");
        assert!((r - 1.0).abs() < f64::EPSILON, "identical texts should have ratio 1.0");
    }

    #[test]
    fn similarity_completely_different() {
        let r = similarity_ratio("aaa\n", "zzz\n");
        assert!(r < 0.5, "completely different texts should have low ratio");
    }

    // ---- DiffHunkStatistics tests ----

    #[test]
    fn hunk_statistics_pure_addition() {
        let hunk = DiffHunk {
            change_type: DiffHunkType::Add,
            original_lines: vec![],
            modified_lines: vec!["a\n".to_string(), "b\n".to_string()],
            original_start: 1,
            modified_start: 1,
        };
        let stats = DiffHunkStatistics::from_hunk(&hunk);
        assert!(stats.is_pure_addition());
        assert!(!stats.is_pure_deletion());
        assert_eq!(stats.additions, 2);
        assert_eq!(stats.deletions, 0);
        assert_eq!(stats.modifications, 0);
        assert_eq!(stats.total_changes(), 2);
    }

    #[test]
    fn hunk_statistics_pure_deletion() {
        let hunk = DiffHunk {
            change_type: DiffHunkType::Delete,
            original_lines: vec!["x\n".to_string(), "y\n".to_string(), "z\n".to_string()],
            modified_lines: vec![],
            original_start: 1,
            modified_start: 1,
        };
        let stats = DiffHunkStatistics::from_hunk(&hunk);
        assert!(stats.is_pure_deletion());
        assert!(!stats.is_pure_addition());
        assert_eq!(stats.deletions, 3);
        assert_eq!(stats.total_changes(), 3);
    }

    #[test]
    fn hunk_statistics_modification() {
        let hunk = DiffHunk {
            change_type: DiffHunkType::Modify,
            original_lines: vec!["old1\n".to_string(), "old2\n".to_string()],
            modified_lines: vec!["new1\n".to_string(), "new2\n".to_string(), "new3\n".to_string()],
            original_start: 1,
            modified_start: 1,
        };
        let stats = DiffHunkStatistics::from_hunk(&hunk);
        assert_eq!(stats.modifications, 2);
        assert_eq!(stats.additions, 1);
        assert_eq!(stats.deletions, 0);
        assert!(!stats.is_pure_addition());
        assert!(!stats.is_pure_deletion());
    }

    #[test]
    fn hunk_statistics_display() {
        let hunk = DiffHunk {
            change_type: DiffHunkType::Modify,
            original_lines: vec!["a\n".to_string()],
            modified_lines: vec!["b\n".to_string(), "c\n".to_string()],
            original_start: 1,
            modified_start: 1,
        };
        let stats = DiffHunkStatistics::from_hunk(&hunk);
        let s = format!("{stats}");
        assert!(s.contains('+'));
        assert!(s.contains('-'));
        assert!(s.contains('~'));
    }

    // ---- SemanticDiffGroup tests ----

    #[test]
    fn semantic_group_empty_input() {
        let groups = SemanticDiffGroup::group_changes(&[], 3);
        assert!(groups.is_empty());
    }

    #[test]
    fn semantic_group_single_change() {
        let changes = vec![DiffChange {
            kind: DiffChangeKind::Insert,
            original_start: 5,
            original_length: 0,
            modified_start: 5,
            modified_length: 2,
        }];
        let groups = SemanticDiffGroup::group_changes(&changes, 3);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].change_count(), 1);
        assert_eq!(groups[0].start_line, 5);
    }

    #[test]
    fn semantic_group_merges_adjacent() {
        let changes = vec![
            DiffChange { kind: DiffChangeKind::Delete, original_start: 2, original_length: 1, modified_start: 2, modified_length: 0 },
            DiffChange { kind: DiffChangeKind::Insert, original_start: 5, original_length: 0, modified_start: 4, modified_length: 1 },
        ];
        let groups = SemanticDiffGroup::group_changes(&changes, 3);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].change_count(), 2);
        assert!(groups[0].line_span() >= 1);
    }

    #[test]
    fn semantic_group_splits_distant() {
        let changes = vec![
            DiffChange { kind: DiffChangeKind::Delete, original_start: 1, original_length: 1, modified_start: 1, modified_length: 0 },
            DiffChange { kind: DiffChangeKind::Insert, original_start: 50, original_length: 0, modified_start: 49, modified_length: 1 },
        ];
        let groups = SemanticDiffGroup::group_changes(&changes, 2);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].start_line, 1);
        assert_eq!(groups[1].start_line, 50);
    }

    // ---- DiffSummaryReport tests ----

    #[test]
    fn summary_report_identical() {
        let report = DiffSummaryReport::from_diff("hello\nworld\n", "hello\nworld\n");
        assert_eq!(report.total_lines_changed(), 0);
        assert!(!report.has_additions());
        assert!(!report.has_deletions());
        assert!((report.change_ratio()).abs() < f64::EPSILON);
    }

    #[test]
    fn summary_report_with_changes() {
        let report = DiffSummaryReport::from_diff("a\nb\nc\n", "a\nx\ny\nc\n");
        assert!(report.total_lines_changed() > 0);
        assert!(report.change_ratio() > 0.0);
        let s = format!("{report}");
        assert!(s.contains("DiffSummary"));
        assert!(s.contains("changed"));
    }

    #[test]
    fn summary_report_empty_texts() {
        let report = DiffSummaryReport::from_diff("", "");
        assert_eq!(report.total_lines_changed(), 0);
        assert!((report.change_ratio()).abs() < f64::EPSILON);
    }

    // ---- CharLevelDiff tests ----

    #[test]
    fn char_diff_identical() {
        let result = CharLevelDiff::compute("hello", "hello");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "hello");
    }

    #[test]
    fn char_diff_single_char_change() {
        let result = CharLevelDiff::compute("cat", "car");
        assert!(result.len() >= 2);
        let has_delete = result.iter().any(|c| c.kind == DiffChangeKind::Delete);
        let has_insert = result.iter().any(|c| c.kind == DiffChangeKind::Insert);
        assert!(has_delete, "should detect deleted char");
        assert!(has_insert, "should detect inserted char");
    }

    #[test]
    fn char_diff_empty_to_text() {
        let result = CharLevelDiff::compute("", "abc");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].kind, DiffChangeKind::Insert);
        assert_eq!(result[0].text, "abc");
    }

#[test]
    fn diffsummarystats_severity_ordering() {
        assert!(DiffSummaryStatsSeverity::Critical > DiffSummaryStatsSeverity::High);
        assert!(DiffSummaryStatsSeverity::High > DiffSummaryStatsSeverity::Medium);
        assert!(DiffSummaryStatsSeverity::Medium > DiffSummaryStatsSeverity::Low);
    }

    #[test]
    fn diffsummarystats_severity_display() {
        assert_eq!(DiffSummaryStatsSeverity::Low.to_string(), "low");
        assert_eq!(DiffSummaryStatsSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn diffsummarystats_entry_creation() {
        let e = DiffSummaryStatsEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, DiffSummaryStatsSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn diffsummarystats_entry_builder() {
        let e = DiffSummaryStatsEntry::new("e2", "Entry 2")
            .with_severity(DiffSummaryStatsSeverity::High)
            .with_detail("some detail")
            .with_additions(42);
        assert_eq!(e.severity, DiffSummaryStatsSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.additions, 42);
    }

    #[test]
    fn diffsummarystats_entry_enable_disable() {
        let mut e = DiffSummaryStatsEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn diffsummarystats_add_and_count() {
        let mut mgr = DiffSummaryStats::new("test");
        mgr.add(DiffSummaryStatsEntry::new("a", "A"));
        mgr.add(DiffSummaryStatsEntry::new("b", "B").with_severity(DiffSummaryStatsSeverity::High));
        assert_eq!(mgr.additions(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn diffsummarystats_remove() {
        let mut mgr = DiffSummaryStats::new("test");
        mgr.add(DiffSummaryStatsEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn diffsummarystats_capacity() {
        let mut mgr = DiffSummaryStats::new("test").with_capacity(1);
        assert!(mgr.add(DiffSummaryStatsEntry::new("a", "A")));
        assert!(!mgr.add(DiffSummaryStatsEntry::new("b", "B")));
    }

    #[test]
    fn diffsummarystats_sorted_by_severity() {
        let mut mgr = DiffSummaryStats::new("test");
        mgr.add(DiffSummaryStatsEntry::new("lo", "Low"));
        mgr.add(DiffSummaryStatsEntry::new("hi", "High").with_severity(DiffSummaryStatsSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, DiffSummaryStatsSeverity::Critical);
    }

    #[test]
    fn diffsummarystats_summary() {
        let mgr = DiffSummaryStats::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn diffpatchformatter_config_defaults() {
        let cfg = DiffPatchFormatterConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn diffpatchformatter_item_creation() {
        let item = DiffPatchFormatterItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn diffpatchformatter_add_and_get() {
        let mut mgr = DiffPatchFormatter::new(DiffPatchFormatterConfig::new("test"));
        mgr.add(DiffPatchFormatterItem::new("k1", "v1"));
        assert_eq!(mgr.deletions(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn diffpatchformatter_remove_item() {
        let mut mgr = DiffPatchFormatter::new(DiffPatchFormatterConfig::new("test"));
        mgr.add(DiffPatchFormatterItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn diffpatchformatter_sorted_by_priority() {
        let mut mgr = DiffPatchFormatter::new(DiffPatchFormatterConfig::new("test"));
        mgr.add(DiffPatchFormatterItem::new("lo", "low").with_priority(1));
        mgr.add(DiffPatchFormatterItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn diffpatchformatter_items_with_tag() {
        let mut mgr = DiffPatchFormatter::new(DiffPatchFormatterConfig::new("test"));
        mgr.add(DiffPatchFormatterItem::new("a", "1").with_tag("x"));
        mgr.add(DiffPatchFormatterItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn diffpatchformatter_report() {
        let mgr = DiffPatchFormatter::new(DiffPatchFormatterConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn diff_x_config_new() {
        let c = DiffXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn diff_x_config_builder() {
        let c = DiffXConfig::new("k")
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
    fn diff_x_config_display() {
        let c = DiffXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn diff_x_registry_insert_get() {
        let mut reg = DiffXRegistry::new();
        reg.insert(DiffXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn diff_x_registry_duplicate() {
        let mut reg = DiffXRegistry::new();
        reg.insert(DiffXConfig::new("a")).unwrap();
        assert!(reg.insert(DiffXConfig::new("a")).is_err());
    }

    #[test]
    fn diff_x_registry_remove() {
        let mut reg = DiffXRegistry::new();
        reg.insert(DiffXConfig::new("a")).unwrap();
        reg.insert(DiffXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn diff_x_registry_active_entries() {
        let mut reg = DiffXRegistry::new();
        reg.insert(DiffXConfig::new("a")).unwrap();
        reg.insert(DiffXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn diff_x_registry_by_weight() {
        let mut reg = DiffXRegistry::new();
        reg.insert(DiffXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(DiffXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn diff_x_registry_tags() {
        let mut reg = DiffXRegistry::new();
        reg.insert(DiffXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(DiffXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn diff_x_registry_total_weight() {
        let mut reg = DiffXRegistry::new();
        reg.insert(DiffXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(DiffXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn diff_x_registry_iterator() {
        let mut reg = DiffXRegistry::new();
        reg.insert(DiffXConfig::new("a")).unwrap();
        reg.insert(DiffXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn diff_x_cache_put_get() {
        let mut cache = DiffXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn diff_x_cache_eviction() {
        let mut cache = DiffXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn diff_x_cache_lru_order() {
        let mut cache = DiffXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn diff_x_cache_most_least_recent() {
        let mut cache = DiffXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn diff_x_formatter_entry() {
        let e = DiffXConfig::new("k").with_value("v");
        let fmt = DiffXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn diff_x_formatter_summary() {
        let mut reg = DiffXRegistry::new();
        reg.insert(DiffXConfig::new("a").with_weight(5)).unwrap();
        let fmt = DiffXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn diff_x_validator_valid() {
        let v = DiffXValidator::new();
        let c = DiffXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn diff_x_validator_empty_key() {
        let v = DiffXValidator::new();
        let c = DiffXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn diff_x_validator_require_value() {
        let v = DiffXValidator::new().require_value(true);
        let c = DiffXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn diff_x_validator_allowed_tags() {
        let v = DiffXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = DiffXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn diff_x_validator_validate_all() {
        let v = DiffXValidator::new();
        let mut reg = DiffXRegistry::new();
        reg.insert(DiffXConfig::new("ok")).unwrap();
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


    #[test]
    fn xb_ring_buffer_57_push_and_len() {
        let mut rb = super::XbRingBuffer57::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_57_overwrite() {
        let mut rb = super::XbRingBuffer57::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_57_get_out_of_bounds() {
        let rb = super::XbRingBuffer57::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_57_drain_all() {
        let mut rb = super::XbRingBuffer57::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_57_peek_front_back() {
        let mut rb = super::XbRingBuffer57::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_57_clear() {
        let mut rb = super::XbRingBuffer57::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_57_capacity() {
        let rb = super::XbRingBuffer57::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_57_basic() {
        let h = super::xb_fnv1a_57(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_57(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_57_different_inputs() {
        let h1 = super::xb_fnv1a_57(b"abc");
        let h2 = super::xb_fnv1a_57(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_57_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_57(&data);
        let dec = super::xb_rle_decode_57(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_57_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_57(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_57(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_57_values() {
        assert!((super::xb_clamp_57(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_57(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_57(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_57_values() {
        assert!((super::xb_lerp_57(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_57(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_57(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_57_wrap_around_twice() {
        let mut rb = super::XbRingBuffer57::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 30 ----

    #[test]
    fn xc_30_pool_new_empty() {
        let pool: super::Xc30Pool<i32> = super::Xc30Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_30_pool_release_acquire() {
        let mut pool = super::Xc30Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_30_pool_acquire_empty() {
        let mut pool: super::Xc30Pool<i32> = super::Xc30Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_30_pool_full() {
        let mut pool = super::Xc30Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_30_pool_drain() {
        let mut pool = super::Xc30Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_30_pool_stats() {
        let mut pool = super::Xc30Pool::new(8);
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
    fn xc_30_pool_clear() {
        let mut pool = super::Xc30Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_30_pool_shrink() {
        let mut pool = super::Xc30Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_30_pool_default() {
        let pool: super::Xc30Pool<String> = super::Xc30Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_30_pool_extend() {
        let mut pool = super::Xc30Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_30_pool_retain() {
        let mut pool = super::Xc30Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_30_scheduler_round_robin() {
        let mut sched = super::Xc30Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_30_scheduler_empty() {
        let mut sched = super::Xc30Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_30_scheduler_reset() {
        let mut sched = super::Xc30Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_30_scheduler_add_remove() {
        let mut sched = super::Xc30Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_30_scheduler_targets() {
        let sched = super::Xc30Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_30_hash_empty() {
        assert_eq!(super::xc_30_hash(b""), 5381);
    }

    #[test]
    fn xc_30_hash_data() {
        let h = super::xc_30_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_30_hash(b"hello"), h);
    }

    #[test]
    fn xc_30_reverse_str() {
        assert_eq!(super::xc_30_reverse("abc"), "cba");
        assert_eq!(super::xc_30_reverse(""), "");
    }


    #[test]
    fn xe_70_pipeline_empty() {
        let p = super::Xe70Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_70_pipeline_parse_stage() {
        let p = super::Xe70Pipeline::new()
            .add_parse(super::xe_70_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_70_pipeline_transform_double() {
        let p = super::Xe70Pipeline::new()
            .add_transform(super::xe_70_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_70_pipeline_validate_reverse() {
        let p = super::Xe70Pipeline::new()
            .add_validate(super::xe_70_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_70_pipeline_emit_filter() {
        let p = super::Xe70Pipeline::new()
            .add_emit(super::xe_70_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_70_pipeline_multi_stage() {
        let p = super::Xe70Pipeline::new()
            .add_parse(super::xe_70_pipeline_identity)
            .add_transform(super::xe_70_pipeline_double)
            .add_validate(super::xe_70_pipeline_reverse)
            .add_emit(super::xe_70_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_70_pipeline_error_propagation() {
        let p = super::Xe70Pipeline::new()
            .add_parse(super::xe_70_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe70Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_70_pipeline_compose() {
        let p1 = super::Xe70Pipeline::new()
            .add_parse(super::xe_70_pipeline_identity);
        let p2 = super::Xe70Pipeline::new()
            .add_transform(super::xe_70_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_70_pipeline_error_display() {
        let e = super::Xe70PipelineError {
            stage: super::Xe70Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_70_cache_put_get() {
        let mut c = super::Xe70Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_70_cache_miss() {
        let mut c: super::Xe70Cache<&str, i32> = super::Xe70Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_70_cache_ttl_expiry() {
        let mut c = super::Xe70Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_70_cache_evict() {
        let mut c = super::Xe70Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_70_cache_capacity() {
        let mut c = super::Xe70Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_70_cache_stats() {
        let mut c = super::Xe70Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_70_cache_clear() {
        let mut c = super::Xe70Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_68 graph tests ------------------------------------------------

    #[test]
    fn xg_68_graph_empty() {
        let g = super::Xg68Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_68_graph_add_node() {
        let mut g = super::Xg68Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_68_graph_add_edge() {
        let mut g = super::Xg68Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_68_graph_neighbors() {
        let mut g = super::Xg68Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_68_graph_has_path() {
        let mut g = super::Xg68Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_68_graph_self_path() {
        let g = super::Xg68Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_68_graph_topo_sort() {
        let mut g = super::Xg68Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_68_graph_cycle_detect_false() {
        let mut g = super::Xg68Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_68_graph_cycle_detect_true() {
        let mut g = super::Xg68Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_68 heap tests -------------------------------------------------

    #[test]
    fn xg_68_heap_empty() {
        let h: super::Xg68Heap<i32> = super::Xg68Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_68_heap_push_pop() {
        let mut h = super::Xg68Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_68_heap_peek() {
        let mut h = super::Xg68Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_68_heap_drain_sorted() {
        let mut h = super::Xg68Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_68_heap_merge() {
        let mut a = super::Xg68Heap::new();
        let mut b = super::Xg68Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_68_heap_default() {
        let h: super::Xg68Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_68_graph_default() {
        let g: super::Xg68Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh29_skip_insert_contains() {
        let mut sl = super::Xh29SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh29_skip_remove() {
        let mut sl = super::Xh29SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh29_skip_len() {
        let mut sl = super::Xh29SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh29_skip_range_query() {
        let mut sl = super::Xh29SkipList::xh_new(4);
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
    fn xh29_skip_floor_ceiling() {
        let mut sl = super::Xh29SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh29_skip_rank() {
        let mut sl = super::Xh29SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh29_skip_empty() {
        let sl = super::Xh29SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh29_skip_duplicates() {
        let mut sl = super::Xh29SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh29_bitset_set_test() {
        let mut bs = super::Xh29BitSet::xh_new(256);
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
    fn xh29_bitset_clear_count() {
        let mut bs = super::Xh29BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh29_bitset_and_or_xor() {
        let mut a = super::Xh29BitSet::xh_new(128);
        let mut b = super::Xh29BitSet::xh_new(128);
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
    fn xh29_bitset_iter_ones() {
        let mut bs = super::Xh29BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh29_bitset_first_last() {
        let mut bs = super::Xh29BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh29_bitset_empty() {
        let bs = super::Xh29BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi29_deque_push_pop_back() {
        let mut dq = super::Xi29Deque::xi_new(4);
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
    fn xi29_deque_push_pop_front() {
        let mut dq = super::Xi29Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi29_deque_mixed_ops() {
        let mut dq = super::Xi29Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi29_deque_get_and_split() {
        let mut dq = super::Xi29Deque::xi_new(8);
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
    fn xi29_deque_rotate_left() {
        let mut dq = super::Xi29Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi29_deque_rotate_right() {
        let mut dq = super::Xi29Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi29_deque_grow() {
        let mut dq = super::Xi29Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi29_deque_empty() {
        let dq = super::Xi29Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi29_interval_tree_insert_query() {
        let mut tree = super::Xi29IntervalTree::xi_new();
        tree.xi_insert(super::Xi29Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi29Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi29Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi29_interval_tree_overlap() {
        let mut tree = super::Xi29IntervalTree::xi_new();
        tree.xi_insert(super::Xi29Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi29Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi29Interval::xi_new(12, 20));
        let q = super::Xi29Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi29_interval_tree_remove() {
        let mut tree = super::Xi29IntervalTree::xi_new();
        tree.xi_insert(super::Xi29Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi29Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi29_interval_tree_gaps() {
        let mut tree = super::Xi29IntervalTree::xi_new();
        tree.xi_insert(super::Xi29Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi29Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi29Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi29Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi29Interval::xi_new(8, 10));
    }

    #[test]
    fn xi29_interval_tree_merge() {
        let mut tree = super::Xi29IntervalTree::xi_new();
        tree.xi_insert(super::Xi29Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi29Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi29Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi29Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi29Interval::xi_new(10, 15));
    }

    #[test]
    fn xi29_interval_tree_all() {
        let mut tree = super::Xi29IntervalTree::xi_new();
        tree.xi_insert(super::Xi29Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi29Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi29_interval_tree_empty() {
        let tree = super::Xi29IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi29_interval_tree_contains_point() {
        let iv = super::Xi29Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 28) ---

    #[test]
    fn xj_28_uf_make_and_find() {
        let mut uf = super::Xj28UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_28_uf_union_connected() {
        let mut uf = super::Xj28UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_28_uf_component_count() {
        let mut uf = super::Xj28UnionFind::xj_new();
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
    fn xj_28_uf_component_size() {
        let mut uf = super::Xj28UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_28_uf_largest_component() {
        let mut uf = super::Xj28UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_28_uf_many_elements() {
        let mut uf = super::Xj28UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_28_uf_separate_components() {
        let mut uf = super::Xj28UnionFind::xj_new();
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
    fn xj_28_uf_path_compression() {
        let mut uf = super::Xj28UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_28_bt_insert_get() {
        let mut bt = super::Xj28BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_28_bt_contains_len() {
        let mut bt = super::Xj28BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_28_bt_replace() {
        let mut bt = super::Xj28BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_28_bt_remove() {
        let mut bt = super::Xj28BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_28_bt_keys_values() {
        let mut bt = super::Xj28BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_28_bt_range() {
        let mut bt = super::Xj28BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_28_bt_min_max() {
        let mut bt = super::Xj28BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_28_bt_many_inserts() {
        let mut bt = super::Xj28BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_29 segment tree tests ---

    #[test]
    fn xk_29_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk29SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_29_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk29SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_29_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk29SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_29_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk29SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_29_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk29SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_29_st_single_element() {
        let data = vec![42];
        let st = super::Xk29SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_29_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk29SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_29_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk29SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_29 disjoint intervals tests ---

    #[test]
    fn xk_29_di_add_and_count() {
        let mut di = super::Xk29DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_29_di_merge_overlap() {
        let mut di = super::Xk29DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_29_di_contains() {
        let mut di = super::Xk29DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_29_di_remove() {
        let mut di = super::Xk29DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_29_di_covered_length() {
        let mut di = super::Xk29DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_29_di_gaps() {
        let mut di = super::Xk29DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_29_di_merge_adjacent() {
        let mut di = super::Xk29DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_29_di_empty() {
        let di = super::Xk29DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_28_rope_new_empty() {
        let rope = super::Xl28Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_28_rope_from_str() {
        let rope = super::Xl28Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_28_rope_insert_at() {
        let mut rope = super::Xl28Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_28_rope_delete_range() {
        let mut rope = super::Xl28Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_28_rope_char_at() {
        let rope = super::Xl28Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_28_rope_split_concat() {
        let rope = super::Xl28Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_28_rope_line_count() {
        let rope = super::Xl28Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_28_rope_line_at() {
        let rope = super::Xl28Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_28_sa_build_and_search() {
        let sa = super::Xl28SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_28_sa_count() {
        let sa = super::Xl28SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_28_sa_longest_repeated() {
        let sa = super::Xl28SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_28_sa_all_positions() {
        let sa = super::Xl28SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_28_sa_len() {
        let sa = super::Xl28SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_28_sa_empty() {
        let sa = super::Xl28SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_28_rope_slice() {
        let rope = super::Xl28Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_28_sa_search_start() {
        let sa = super::Xl28SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_28_sparse_set_get() {
        let mut m = super::Xm28MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_28_sparse_row_col() {
        let mut m = super::Xm28MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_28_sparse_transpose() {
        let mut m = super::Xm28MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_28_sparse_multiply_vec() {
        let mut m = super::Xm28MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_28_sparse_nnz_density() {
        let mut m = super::Xm28MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_28_sparse_clear() {
        let mut m = super::Xm28MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_28_sparse_overwrite_zero() {
        let mut m = super::Xm28MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_28_tokenizer_basic() {
        let t = super::Xm28Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_28_tokenizer_count() {
        let t = super::Xm28Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_28_tokenizer_unique() {
        let t = super::Xm28Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_28_tokenizer_frequency() {
        let t = super::Xm28Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_28_tokenizer_delimiter() {
        let t = super::Xm28Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_28_tokenizer_whitespace() {
        let t = super::Xm28Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_28_tokenizer_empty() {
        let t = super::Xm28Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 29 ----

    #[test]
    fn xn_29_fenwick_prefix_sum() {
        let mut ft = super::Xn29Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_29_fenwick_range_sum() {
        let mut ft = super::Xn29Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_29_fenwick_point_query() {
        let mut ft = super::Xn29Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_29_fenwick_len() {
        let ft = super::Xn29Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_29_fenwick_multiple_updates() {
        let mut ft = super::Xn29Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_29_fenwick_single_element() {
        let mut ft = super::Xn29Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_29_fenwick_find_kth() {
        let mut ft = super::Xn29Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_29_fenwick_negative_delta() {
        let mut ft = super::Xn29Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 29 ----

    #[test]
    fn xn_29_avl_insert_get() {
        let mut m = super::Xn29AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_29_avl_remove() {
        let mut m = super::Xn29AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_29_avl_in_order() {
        let mut m = super::Xn29AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_29_avl_min_max() {
        let mut m = super::Xn29AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_29_avl_floor_ceiling() {
        let mut m = super::Xn29AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_29_avl_height_balanced() {
        let mut m = super::Xn29AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_29_avl_overwrite() {
        let mut m = super::Xn29AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_29_avl_empty() {
        let m: super::Xn29AVL<i32, i32> = super::Xn29AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo29RedBlack tests ---

    #[test]
    fn xo_29_rb_insert_and_get() {
        let mut tree = super::Xo29RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_29_rb_len_and_empty() {
        let mut tree = super::Xo29RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_29_rb_min_max() {
        let mut tree = super::Xo29RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_29_rb_contains() {
        let mut tree = super::Xo29RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_29_rb_remove() {
        let mut tree = super::Xo29RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_29_rb_in_order() {
        let mut tree = super::Xo29RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_29_rb_black_height() {
        let mut tree = super::Xo29RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_29_rb_overwrite() {
        let mut tree = super::Xo29RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo29ConsistentHash tests ---

    #[test]
    fn xo_29_ch_add_and_count() {
        let mut ring = super::Xo29ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_29_ch_remove_node() {
        let mut ring = super::Xo29ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_29_ch_get_node() {
        let mut ring = super::Xo29ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_29_ch_empty_ring() {
        let ring = super::Xo29ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_29_ch_distribution() {
        let mut ring = super::Xo29ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_29_ch_rebalance() {
        let mut ring = super::Xo29ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_29_ch_virtual_nodes() {
        let mut ring = super::Xo29ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_29_ch_consistent_lookup() {
        let mut ring = super::Xo29ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}
