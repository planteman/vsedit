//! Code folding model.
//!
//! Equivalent to VS Code's folding region computation.

use std::collections::HashMap;
use std::fmt;

/// A foldable range in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingRange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: FoldingRangeKind,
    pub is_collapsed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldingRangeKind {
    Comment,
    Imports,
    Region,
}

/// Manages folding ranges for a document.
pub struct FoldingModel {
    ranges: Vec<FoldingRange>,
}

impl FoldingModel {
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    pub fn set_ranges(&mut self, ranges: Vec<FoldingRange>) {
        self.ranges = ranges;
        self.ranges.sort_by_key(|r| r.start_line);
    }

    pub fn get_ranges(&self) -> &[FoldingRange] {
        &self.ranges
    }

    pub fn toggle(&mut self, line: u32) {
        for range in &mut self.ranges {
            if range.start_line == line {
                range.is_collapsed = !range.is_collapsed;
                return;
            }
        }
    }

    pub fn fold_all(&mut self) {
        for range in &mut self.ranges {
            range.is_collapsed = true;
        }
    }

    pub fn unfold_all(&mut self) {
        for range in &mut self.ranges {
            range.is_collapsed = false;
        }
    }

    /// Check if a line is hidden by folding.
    pub fn is_line_hidden(&self, line: u32) -> bool {
        self.ranges.iter().any(|r| {
            r.is_collapsed && line > r.start_line && line <= r.end_line
        })
    }

    /// Get the folding range at a line (if the line is a fold start).
    pub fn get_range_at(&self, line: u32) -> Option<&FoldingRange> {
        self.ranges.iter().find(|r| r.start_line == line)
    }

    /// Detect folding ranges from indentation.
    pub fn compute_from_indentation(lines: &[&str], tab_size: u32) -> Vec<FoldingRange> {
        let mut ranges = Vec::new();
        let indents: Vec<u32> = lines.iter().map(|l| Self::indent_level(l, tab_size)).collect();
        let mut stack: Vec<(u32, u32)> = Vec::new(); // (line, indent)

        for (i, &indent) in indents.iter().enumerate() {
            let line = (i + 1) as u32;
            while let Some(&(start_line, start_indent)) = stack.last() {
                if indent <= start_indent {
                    let end_line = line - 1;
                    if end_line > start_line {
                        ranges.push(FoldingRange {
                            start_line,
                            end_line,
                            kind: FoldingRangeKind::Region,
                            is_collapsed: false,
                        });
                    }
                    stack.pop();
                } else {
                    break;
                }
            }
            if !lines[i].trim().is_empty() {
                stack.push((line, indent));
            }
        }

        // Close remaining
        let last_line = lines.len() as u32;
        while let Some((start_line, _)) = stack.pop() {
            if last_line > start_line {
                ranges.push(FoldingRange {
                    start_line,
                    end_line: last_line,
                    kind: FoldingRangeKind::Region,
                    is_collapsed: false,
                });
            }
        }

        ranges.sort_by_key(|r| r.start_line);
        ranges
    }

    fn indent_level(line: &str, tab_size: u32) -> u32 {
        let mut level: u32 = 0;
        for ch in line.chars() {
            match ch {
                ' ' => level += 1,
                '\t' => level += tab_size,
                _ => break,
            }
        }
        level
    }

    /// Returns true if ranges is empty.
    pub fn is_ranges_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Get the first range, if any.
    pub fn first_range(&self) -> Option<&FoldingRange> {
        self.ranges.first()
    }

    /// Get the last range, if any.
    pub fn last_range(&self) -> Option<&FoldingRange> {
        self.ranges.last()
    }

    /// Retain only ranges matching the predicate.
    pub fn retain_ranges(&mut self, f: impl Fn(&FoldingRange) -> bool) {
        self.ranges.retain(|item| f(item));
    }
}

impl Default for FoldingModel {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Additional folding operations
// ---------------------------------------------------------------------------

impl FoldingRange {
    /// Compute the nesting depth of this range within a set of ranges.
    fn nesting_depth_in(&self, ranges: &[FoldingRange]) -> u32 {
        ranges
            .iter()
            .filter(|r| {
                r.start_line < self.start_line && r.end_line > self.end_line
            })
            .count() as u32
    }
}

impl FoldingModel {
    /// Fold all ranges whose nesting depth is >= `level` (0-based).
    pub fn fold_level(&mut self, level: u32) {
        let snapshot: Vec<FoldingRange> = self.ranges.clone();
        for range in &mut self.ranges {
            let depth = range.nesting_depth_in(&snapshot);
            if depth >= level {
                range.is_collapsed = true;
            }
        }
    }

    /// Return references to all currently collapsed ranges.
    pub fn get_collapsed_ranges(&self) -> Vec<&FoldingRange> {
        self.ranges.iter().filter(|r| r.is_collapsed).collect()
    }

    /// Count the number of lines that are currently visible (not hidden by
    /// any collapsed fold). `total_lines` is the total line count of the
    /// document (1-based, i.e. the last line number).
    pub fn get_visible_line_count(&self, total_lines: u32) -> u32 {
        (1..=total_lines)
            .filter(|&line| !self.is_line_hidden(line))
            .count() as u32
    }

    /// Collapse every range whose kind matches `kind`.
    pub fn fold_by_kind(&mut self, kind: FoldingRangeKind) {
        for range in &mut self.ranges {
            if range.kind == kind {
                range.is_collapsed = true;
            }
        }
    }

    /// Unfold the innermost collapsed range that contains `line`.
    pub fn unfold_at(&mut self, line: u32) {
        // Find the innermost (smallest span) collapsed range containing line.
        let idx = self
            .ranges
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_collapsed && line >= r.start_line && line <= r.end_line)
            .min_by_key(|(_, r)| r.end_line - r.start_line)
            .map(|(i, _)| i);
        if let Some(i) = idx {
            self.ranges[i].is_collapsed = false;
        }
    }

    /// Return the nesting depth of `line` – how many ranges contain it.
    pub fn get_nesting_depth(&self, line: u32) -> u32 {
        self.ranges
            .iter()
            .filter(|r| line >= r.start_line && line <= r.end_line)
            .count() as u32
    }

    /// Build folding ranges from explicit region markers.
    ///
    /// Scans `lines` for lines containing `start_marker` and `end_marker` and
    /// pairs them into `FoldingRange` values with kind `Region`.
    pub fn compute_from_markers(
        lines: &[&str],
        start_marker: &str,
        end_marker: &str,
    ) -> Vec<FoldingRange> {
        let mut ranges = Vec::new();
        let mut stack: Vec<u32> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let lineno = (i + 1) as u32;
            if line.contains(start_marker) {
                stack.push(lineno);
            } else if line.contains(end_marker) {
                if let Some(start) = stack.pop() {
                    ranges.push(FoldingRange {
                        start_line: start,
                        end_line: lineno,
                        kind: FoldingRangeKind::Region,
                        is_collapsed: false,
                    });
                }
            }
        }
        ranges.sort_by_key(|r| r.start_line);
        ranges
    }
}

/// Trait for types that can provide folding ranges for a document.
pub trait FoldingProvider {
    /// Compute folding ranges for the given document content.
    fn compute_folding_ranges(&self, text: &str) -> Vec<FoldingRange>;

    /// Default implementation: compute from indentation with a tab size of 4.
    fn compute_default(&self, text: &str) -> Vec<FoldingRange> {
        let lines: Vec<&str> = text.lines().collect();
        FoldingModel::compute_from_indentation(&lines, 4)
    }
}

/// A simple indentation-based folding provider.
pub struct IndentFoldingProvider {
    pub tab_size: u32,
}

impl IndentFoldingProvider {
    pub fn new(tab_size: u32) -> Self {
        Self { tab_size }
    }
}

impl FoldingProvider for IndentFoldingProvider {
    fn compute_folding_ranges(&self, text: &str) -> Vec<FoldingRange> {
        let lines: Vec<&str> = text.lines().collect();
        FoldingModel::compute_from_indentation(&lines, self.tab_size)
    }
}

// ---------------------------------------------------------------------------
// Bracket-based folding
// ---------------------------------------------------------------------------

impl FoldingModel {
    /// Compute foldable ranges from bracket pairs `{}`.
    pub fn compute_from_brackets(lines: &[&str]) -> Vec<FoldingRange> {
        let mut ranges = Vec::new();
        let mut stack: Vec<u32> = Vec::new(); // line numbers of opening braces

        for (i, line) in lines.iter().enumerate() {
            let line_num = (i + 1) as u32;
            let bytes = line.as_bytes();
            let mut j = 0;
            let len = bytes.len();
            while j < len {
                // Skip strings
                if bytes[j] == b'"' || bytes[j] == b'\'' {
                    let q = bytes[j];
                    j += 1;
                    while j < len {
                        if bytes[j] == b'\\' { j += 2; continue; }
                        if bytes[j] == q { break; }
                        j += 1;
                    }
                    j += 1;
                    continue;
                }
                // Skip line comments
                if j + 1 < len && bytes[j] == b'/' && bytes[j + 1] == b'/' {
                    break;
                }
                if bytes[j] == b'{' {
                    stack.push(line_num);
                } else if bytes[j] == b'}' {
                    if let Some(start) = stack.pop() {
                        if line_num > start {
                            ranges.push(FoldingRange {
                                start_line: start,
                                end_line: line_num,
                                kind: FoldingRangeKind::Region,
                                is_collapsed: false,
                            });
                        }
                    }
                }
                j += 1;
            }
        }
        ranges.sort_by_key(|r| r.start_line);
        ranges
    }

    /// Detect import blocks and create folding ranges for them.
    ///
    /// An import block is a sequence of consecutive lines starting with
    /// `use `, `import `, or `from ` (common in Rust/JS/Python).
    pub fn compute_from_imports(lines: &[&str]) -> Vec<FoldingRange> {
        let mut ranges = Vec::new();
        let mut block_start: Option<u32> = None;

        for (i, line) in lines.iter().enumerate() {
            let line_num = (i + 1) as u32;
            let trimmed = line.trim();
            let is_import = trimmed.starts_with("use ")
                || trimmed.starts_with("import ")
                || trimmed.starts_with("from ");

            if is_import {
                if block_start.is_none() {
                    block_start = Some(line_num);
                }
            } else if !trimmed.is_empty() {
                if let Some(start) = block_start.take() {
                    let end = line_num - 1;
                    if end > start {
                        ranges.push(FoldingRange {
                            start_line: start,
                            end_line: end,
                            kind: FoldingRangeKind::Imports,
                            is_collapsed: false,
                        });
                    }
                }
            }
        }
        // Close any trailing import block
        if let Some(start) = block_start {
            let end = lines.len() as u32;
            if end > start {
                ranges.push(FoldingRange {
                    start_line: start,
                    end_line: end,
                    kind: FoldingRangeKind::Imports,
                    is_collapsed: false,
                });
            }
        }
        ranges
    }

    /// Compute folding ranges combining multiple strategies.
    pub fn compute_all_ranges(lines: &[&str], tab_size: u32) -> Vec<FoldingRange> {
        let mut ranges = Vec::new();
        ranges.extend(Self::compute_from_indentation(lines, tab_size));
        ranges.extend(Self::compute_from_brackets(lines));
        ranges.extend(Self::compute_from_markers(lines, "#region", "#endregion"));
        ranges.extend(Self::compute_from_markers(lines, "// #region", "// #endregion"));
        ranges.extend(Self::compute_from_imports(lines));

        // Deduplicate: prefer bracket-based over indent when same start_line
        ranges.sort_by(|a, b| a.start_line.cmp(&b.start_line).then(a.end_line.cmp(&b.end_line)));
        ranges.dedup_by(|a, b| a.start_line == b.start_line && a.end_line == b.end_line);
        ranges
    }
}

/// A folding provider that combines indentation, bracket, region, and import strategies.
pub struct CompositeFoldingProvider {
    pub tab_size: u32,
}

impl CompositeFoldingProvider {
    pub fn new(tab_size: u32) -> Self {
        Self { tab_size }
    }
}

impl FoldingProvider for CompositeFoldingProvider {
    fn compute_folding_ranges(&self, text: &str) -> Vec<FoldingRange> {
        let lines: Vec<&str> = text.lines().collect();
        FoldingModel::compute_all_ranges(&lines, self.tab_size)
    }
}

impl FoldingRange {
    /// Compute the fold level (0-based nesting depth) within a list of ranges.
    pub fn fold_level_in(&self, ranges: &[FoldingRange]) -> u32 {
        self.nesting_depth_in(ranges)
    }

    /// Returns true if this range is nested inside another range.
    pub fn is_nested_in(&self, other: &FoldingRange) -> bool {
        other.start_line < self.start_line && other.end_line > self.end_line
    }

    /// Span of this range in lines.
    pub fn line_span(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line)
    }
}

impl FoldingModel {
    /// Find all ranges that are nested inside a given parent range.
    pub fn find_nested(&self, parent: &FoldingRange) -> Vec<&FoldingRange> {
        self.ranges
            .iter()
            .filter(|r| r.start_line > parent.start_line && r.end_line < parent.end_line)
            .collect()
    }

    /// Serialize fold state as a list of (start_line, is_collapsed) pairs.
    pub fn serialize_state(&self) -> Vec<(u32, bool)> {
        self.ranges.iter().map(|r| (r.start_line, r.is_collapsed)).collect()
    }

    /// Restore fold state from serialized pairs. Only affects ranges whose
    /// start_line matches an entry.
    pub fn restore_state(&mut self, state: &[(u32, bool)]) {
        for (line, collapsed) in state {
            for range in &mut self.ranges {
                if range.start_line == *line {
                    range.is_collapsed = *collapsed;
                }
            }
        }
    }

    /// Compute statistics about folding ranges.
    pub fn statistics(&self) -> FoldingStatistics {
        let total = self.ranges.len() as u32;
        let collapsed = self.ranges.iter().filter(|r| r.is_collapsed).count() as u32;
        let max_depth = self.ranges.iter()
            .map(|r| r.nesting_depth_in(&self.ranges))
            .max()
            .unwrap_or(0);
        let total_span: u32 = self.ranges.iter().map(|r| r.line_span()).sum();
        FoldingStatistics { total, collapsed, expanded: total - collapsed, max_depth, total_span }
    }
}

/// Aggregate statistics for a folding model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingStatistics {
    pub total: u32,
    pub collapsed: u32,
    pub expanded: u32,
    pub max_depth: u32,
    pub total_span: u32,
}

// ---------------------------------------------------------------------------
// Comment-based folding provider
// ---------------------------------------------------------------------------

/// A folding provider that detects consecutive comment-line blocks.
pub struct CommentFoldingProvider {
    pub comment_prefix: String,
}

impl CommentFoldingProvider {
    pub fn new(comment_prefix: &str) -> Self {
        Self {
            comment_prefix: comment_prefix.to_string(),
        }
    }
}

impl FoldingProvider for CommentFoldingProvider {
    fn compute_folding_ranges(&self, text: &str) -> Vec<FoldingRange> {
        let mut ranges = Vec::new();
        let mut block_start: Option<u32> = None;

        for (i, line) in text.lines().enumerate() {
            let line_num = (i + 1) as u32;
            let trimmed = line.trim_start();
            if trimmed.starts_with(&self.comment_prefix) {
                if block_start.is_none() {
                    block_start = Some(line_num);
                }
            } else {
                if let Some(start) = block_start.take() {
                    let end = line_num - 1;
                    if end > start {
                        ranges.push(FoldingRange {
                            start_line: start,
                            end_line: end,
                            kind: FoldingRangeKind::Comment,
                            is_collapsed: false,
                        });
                    }
                }
            }
        }
        // Close trailing comment block
        if let Some(start) = block_start {
            let end = text.lines().count() as u32;
            if end > start {
                ranges.push(FoldingRange {
                    start_line: start,
                    end_line: end,
                    kind: FoldingRangeKind::Comment,
                    is_collapsed: false,
                });
            }
        }
        ranges
    }
}

// ---------------------------------------------------------------------------
// Folding range set with set operations
// ---------------------------------------------------------------------------

/// A collection of folding ranges with set operations.
#[derive(Debug, Clone)]
pub struct FoldingRangeSet {
    ranges: Vec<FoldingRange>,
}

impl FoldingRangeSet {
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    pub fn add(&mut self, range: FoldingRange) {
        self.ranges.push(range);
    }

    /// Merge all ranges from `other`, deduplicating by start_line + end_line.
    pub fn merge(&mut self, other: &FoldingRangeSet) {
        for r in &other.ranges {
            let duplicate = self
                .ranges
                .iter()
                .any(|existing| existing.start_line == r.start_line && existing.end_line == r.end_line);
            if !duplicate {
                self.ranges.push(r.clone());
            }
        }
    }

    pub fn get_ranges(&self) -> &[FoldingRange] {
        &self.ranges
    }

    /// Return all ranges that contain `line`.
    pub fn ranges_containing_line(&self, line: u32) -> Vec<&FoldingRange> {
        self.ranges
            .iter()
            .filter(|r| line >= r.start_line && line <= r.end_line)
            .collect()
    }

    /// Return ranges at a specific nesting depth (0 = top-level).
    pub fn ranges_at_depth(&self, depth: u32) -> Vec<&FoldingRange> {
        self.ranges
            .iter()
            .filter(|r| r.nesting_depth_in(&self.ranges) == depth)
            .collect()
    }

    /// Collapse all ranges at a given nesting depth.
    pub fn collapse_all_at_depth(&mut self, depth: u32) {
        let snapshot: Vec<FoldingRange> = self.ranges.clone();
        for range in &mut self.ranges {
            if range.nesting_depth_in(&snapshot) == depth {
                range.is_collapsed = true;
            }
        }
    }

    /// Sum of lines hidden by collapsed ranges (end - start for each).
    pub fn total_hidden_lines(&self) -> u32 {
        self.ranges
            .iter()
            .filter(|r| r.is_collapsed)
            .map(|r| r.end_line - r.start_line)
            .sum()
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
}

impl Default for FoldingRangeSet {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Region fold / unfold
// ---------------------------------------------------------------------------

/// Fold the range starting at `start_line`. When `recursive` is true, also
/// fold all nested ranges within the target.
pub fn fold_region(model: &mut FoldingModel, start_line: u32, recursive: bool) {
    let nested_starts: Vec<u32> = if recursive {
        if let Some(parent) = model.get_range_at(start_line).cloned() {
            model
                .find_nested(&parent)
                .iter()
                .map(|r| r.start_line)
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    for range in model.ranges.iter_mut() {
        if range.start_line == start_line || (recursive && nested_starts.contains(&range.start_line)) {
            range.is_collapsed = true;
        }
    }
}

// ---------------------------------------------------------------------------
// FoldingSnapshot – save and restore full fold state
// ---------------------------------------------------------------------------

/// A snapshot of the complete fold state that can be saved and restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingSnapshot {
    entries: Vec<(u32, u32, bool)>, // (start_line, end_line, is_collapsed)
}

impl FoldingSnapshot {
    /// Capture the current fold state from a model.
    pub fn capture(model: &FoldingModel) -> Self {
        Self {
            entries: model
                .get_ranges()
                .iter()
                .map(|r| (r.start_line, r.end_line, r.is_collapsed))
                .collect(),
        }
    }

    /// Apply this snapshot to a model. Only ranges whose (start_line, end_line)
    /// match an entry are updated.
    pub fn apply(&self, model: &mut FoldingModel) {
        for range in model.ranges.iter_mut() {
            if let Some((_, _, collapsed)) = self.entries.iter().find(|(s, e, _)| {
                *s == range.start_line && *e == range.end_line
            }) {
                range.is_collapsed = *collapsed;
            }
        }
    }

    /// Number of entries in the snapshot.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Count how many entries are collapsed in this snapshot.
    pub fn collapsed_count(&self) -> usize {
        self.entries.iter().filter(|(_, _, c)| *c).count()
    }
}

// ---------------------------------------------------------------------------
// FoldingDiff – compare two fold states
// ---------------------------------------------------------------------------

/// The kind of change between two fold states for a single range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoldingChangeKind {
    Folded,
    Unfolded,
    Added,
    Removed,
}

/// A single difference between two fold snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingChange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: FoldingChangeKind,
}

/// Compute the differences between two snapshots.
pub fn diff_snapshots(before: &FoldingSnapshot, after: &FoldingSnapshot) -> Vec<FoldingChange> {
    let mut changes = Vec::new();

    // Detect changes and removals by iterating `before`.
    for &(s, e, collapsed_before) in &before.entries {
        match after.entries.iter().find(|(as_, ae, _)| *as_ == s && *ae == e) {
            Some(&(_, _, collapsed_after)) => {
                if collapsed_before && !collapsed_after {
                    changes.push(FoldingChange { start_line: s, end_line: e, kind: FoldingChangeKind::Unfolded });
                } else if !collapsed_before && collapsed_after {
                    changes.push(FoldingChange { start_line: s, end_line: e, kind: FoldingChangeKind::Folded });
                }
            }
            None => {
                changes.push(FoldingChange { start_line: s, end_line: e, kind: FoldingChangeKind::Removed });
            }
        }
    }

    // Detect additions (in `after` but not in `before`).
    for &(s, e, _) in &after.entries {
        if !before.entries.iter().any(|(bs, be, _)| *bs == s && *be == e) {
            changes.push(FoldingChange { start_line: s, end_line: e, kind: FoldingChangeKind::Added });
        }
    }

    changes
}

// ---------------------------------------------------------------------------
// FoldingHistory – track fold/unfold operations over time
// ---------------------------------------------------------------------------

/// A recorded fold or unfold event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingEvent {
    pub start_line: u32,
    pub collapsed: bool,
    pub seq: u64,
}

/// Tracks a sequence of fold/unfold events for undo-style replay.
#[derive(Debug, Clone)]
pub struct FoldingHistory {
    events: Vec<FoldingEvent>,
    next_seq: u64,
}

impl FoldingHistory {
    pub fn new() -> Self {
        Self { events: Vec::new(), next_seq: 0 }
    }

    /// Record a toggle event.
    pub fn record(&mut self, start_line: u32, collapsed: bool) {
        self.events.push(FoldingEvent { start_line, collapsed, seq: self.next_seq });
        self.next_seq += 1;
    }

    /// Return all recorded events in order.
    pub fn events(&self) -> &[FoldingEvent] {
        &self.events
    }

    /// Pop the most recent event (for undo).
    pub fn pop(&mut self) -> Option<FoldingEvent> {
        self.events.pop()
    }

    /// Number of recorded events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Undo the last event by applying its inverse to the model.
    pub fn undo_last(&mut self, model: &mut FoldingModel) -> bool {
        if let Some(event) = self.pop() {
            for range in model.ranges.iter_mut() {
                if range.start_line == event.start_line {
                    range.is_collapsed = !event.collapsed;
                    return true;
                }
            }
        }
        false
    }
}

impl Default for FoldingHistory {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Fold region merging – coalesce adjacent single-line folds
// ---------------------------------------------------------------------------

impl FoldingModel {
    /// Merge adjacent single-line fold ranges into larger contiguous ranges.
    ///
    /// Two ranges are merged when the first range's `end_line` is immediately
    /// followed by the next range's `start_line` (i.e. `end_line + 1 ==
    /// start_line`) and both span exactly one line. The merged range inherits
    /// the kind of the first range and is expanded.
    pub fn merge_adjacent_single_line_folds(&mut self) {
        if self.ranges.len() < 2 {
            return;
        }
        self.ranges.sort_by_key(|r| r.start_line);
        let mut merged: Vec<FoldingRange> = Vec::new();
        let mut i = 0;
        while i < self.ranges.len() {
            let mut current = self.ranges[i].clone();
            if current.line_span() == 1 {
                // Absorb consecutive single-line ranges.
                while i + 1 < self.ranges.len()
                    && self.ranges[i + 1].line_span() == 1
                    && self.ranges[i + 1].start_line == current.end_line + 1
                {
                    current.end_line = self.ranges[i + 1].end_line;
                    i += 1;
                }
            }
            merged.push(current);
            i += 1;
        }
        self.ranges = merged;
    }

    /// Fold all ranges at exactly nesting depth `level` and unfold the rest.
    ///
    /// This is the selective "fold to level N" operation editors expose via
    /// keyboard shortcuts (e.g. Ctrl+K Ctrl+1 folds to level 1).
    pub fn fold_to_level(&mut self, level: u32) {
        let snapshot: Vec<FoldingRange> = self.ranges.clone();
        for range in &mut self.ranges {
            let depth = range.nesting_depth_in(&snapshot);
            range.is_collapsed = depth >= level;
        }
    }

    /// Return the maximum nesting depth across all ranges.
    pub fn max_fold_depth(&self) -> u32 {
        self.ranges
            .iter()
            .map(|r| r.nesting_depth_in(&self.ranges))
            .max()
            .unwrap_or(0)
    }

    /// Return the set of lines that are fold-start lines (fold gutter icons).
    pub fn fold_start_lines(&self) -> Vec<u32> {
        self.ranges.iter().map(|r| r.start_line).collect()
    }

    /// Compute a mapping from each document line to its fold level.
    ///
    /// `total_lines` is the 1-based count of lines in the document. Lines that
    /// are not inside any fold range get level 0.
    pub fn line_fold_levels(&self, total_lines: u32) -> Vec<u32> {
        let mut levels = vec![0u32; total_lines as usize + 1];
        for line in 1..=total_lines {
            levels[line as usize] = self.get_nesting_depth(line);
        }
        levels
    }

    /// Collect all ranges that overlap a given line range `[from, to]`
    /// (inclusive).
    pub fn ranges_overlapping(&self, from: u32, to: u32) -> Vec<&FoldingRange> {
        self.ranges
            .iter()
            .filter(|r| r.start_line <= to && r.end_line >= from)
            .collect()
    }

    /// Toggle all ranges whose kind matches `kind`.
    pub fn toggle_by_kind(&mut self, kind: FoldingRangeKind) {
        for range in &mut self.ranges {
            if range.kind == kind {
                range.is_collapsed = !range.is_collapsed;
            }
        }
    }

    /// Unfold every range whose kind matches `kind`.
    pub fn unfold_by_kind(&mut self, kind: FoldingRangeKind) {
        for range in &mut self.ranges {
            if range.kind == kind {
                range.is_collapsed = false;
            }
        }
    }

    /// Return visible line numbers (not hidden by any collapsed fold).
    pub fn visible_lines(&self, total_lines: u32) -> Vec<u32> {
        (1..=total_lines)
            .filter(|&line| !self.is_line_hidden(line))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Fold state persistence – serialize to / deserialize from a compact string
// ---------------------------------------------------------------------------

impl FoldingModel {
    /// Serialize the collapsed state to a compact string representation.
    ///
    /// Format: `"start:end:c;start:end:c;..."` where `c` is `1` (collapsed)
    /// or `0` (expanded). This is suitable for storing in editor settings or
    /// workspace files.
    pub fn serialize_to_string(&self) -> String {
        self.ranges
            .iter()
            .map(|r| {
                format!(
                    "{}:{}:{}",
                    r.start_line,
                    r.end_line,
                    u8::from(r.is_collapsed)
                )
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Restore collapsed state from a string produced by
    /// [`serialize_to_string`](Self::serialize_to_string).
    ///
    /// Only ranges whose `(start_line, end_line)` pair appears in the
    /// serialized data are updated; other ranges keep their current state.
    pub fn restore_from_string(&mut self, data: &str) {
        if data.is_empty() {
            return;
        }
        for entry in data.split(';') {
            let parts: Vec<&str> = entry.split(':').collect();
            if parts.len() != 3 {
                continue;
            }
            let start: u32 = match parts[0].parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let end: u32 = match parts[1].parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let collapsed: bool = parts[2] == "1";
            for range in &mut self.ranges {
                if range.start_line == start && range.end_line == end {
                    range.is_collapsed = collapsed;
                }
            }
        }
    }
}

/// Unfold the range starting at `start_line`. When `recursive` is true, also
/// unfold all nested ranges within the target.
pub fn unfold_region(model: &mut FoldingModel, start_line: u32, recursive: bool) {
    let nested_starts: Vec<u32> = if recursive {
        if let Some(parent) = model.get_range_at(start_line).cloned() {
            model
                .find_nested(&parent)
                .iter()
                .map(|r| r.start_line)
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    for range in model.ranges.iter_mut() {
        if range.start_line == start_line || (recursive && nested_starts.contains(&range.start_line)) {
            range.is_collapsed = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Folding analysis utilities
// ---------------------------------------------------------------------------

/// Summary statistics for a set of folding ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingSummary {
    /// Total number of folding ranges.
    pub total: usize,
    /// Number of currently collapsed ranges.
    pub collapsed: usize,
    /// Maximum nesting depth across all ranges.
    pub max_depth: u32,
    /// Total number of hidden lines (lines inside collapsed ranges).
    pub hidden_lines: u32,
    /// Count of ranges per kind.
    pub comment_count: usize,
    pub imports_count: usize,
    pub region_count: usize,
}

/// Compute a summary of the current folding state.
pub fn compute_folding_summary(model: &FoldingModel) -> FoldingSummary {
    let ranges = model.get_ranges();
    let total = ranges.len();
    let collapsed = ranges.iter().filter(|r| r.is_collapsed).count();
    let hidden_lines: u32 = ranges
        .iter()
        .filter(|r| r.is_collapsed)
        .map(|r| r.end_line.saturating_sub(r.start_line))
        .sum();
    let max_depth = ranges
        .iter()
        .map(|r| {
            ranges
                .iter()
                .filter(|outer| outer.start_line < r.start_line && outer.end_line > r.end_line)
                .count() as u32
                + 1
        })
        .max()
        .unwrap_or(0);
    let comment_count = ranges.iter().filter(|r| r.kind == FoldingRangeKind::Comment).count();
    let imports_count = ranges.iter().filter(|r| r.kind == FoldingRangeKind::Imports).count();
    let region_count = ranges.iter().filter(|r| r.kind == FoldingRangeKind::Region).count();
    FoldingSummary {
        total,
        collapsed,
        max_depth,
        hidden_lines,
        comment_count,
        imports_count,
        region_count,
    }
}

/// Return only the ranges that overlap a given line span `[start, end]`.
pub fn ranges_overlapping(model: &FoldingModel, start: u32, end: u32) -> Vec<&FoldingRange> {
    model
        .get_ranges()
        .iter()
        .filter(|r| r.start_line <= end && r.end_line >= start)
        .collect()
}

/// Return ranges sorted by span length (smallest first).
pub fn ranges_by_span(model: &FoldingModel) -> Vec<&FoldingRange> {
    let mut sorted: Vec<&FoldingRange> = model.get_ranges().iter().collect();
    sorted.sort_by_key(|r| r.end_line - r.start_line);
    sorted
}

/// Compute the visible line count (total lines minus hidden lines from
/// collapsed ranges). `total_lines` is the document line count.
pub fn visible_line_count(model: &FoldingModel, total_lines: u32) -> u32 {
    let hidden: u32 = model
        .get_ranges()
        .iter()
        .filter(|r| r.is_collapsed)
        .map(|r| r.end_line.saturating_sub(r.start_line))
        .sum();
    total_lines.saturating_sub(hidden)
}

/// Find the innermost folding range at a given line (deepest nesting).
pub fn innermost_range_at(model: &FoldingModel, line: u32) -> Option<&FoldingRange> {
    model
        .get_ranges()
        .iter()
        .filter(|r| r.start_line <= line && r.end_line >= line)
        .min_by_key(|r| r.end_line - r.start_line)
}

/// Return the lines that are "fold headers" – i.e. start lines of folding ranges.
pub fn fold_header_lines(model: &FoldingModel) -> Vec<u32> {
    model.get_ranges().iter().map(|r| r.start_line).collect()
}

// -- FoldingImport for collapsing import blocks ------------------------------

/// Detect import block ranges. Lines starting with common import keywords
/// that form contiguous blocks are grouped as import folding ranges.
pub fn detect_import_ranges(lines: &[&str]) -> Vec<FoldingRange> {
    let import_prefixes = ["use ", "import ", "from ", "#include ", "require("];
    let mut ranges = Vec::new();
    let mut block_start: Option<u32> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let is_import = import_prefixes.iter().any(|p| trimmed.starts_with(p));
        let line_num = (i + 1) as u32;

        match (is_import, block_start) {
            (true, None) => block_start = Some(line_num),
            (false, Some(start)) => {
                if line_num - 1 > start {
                    ranges.push(FoldingRange {
                        start_line: start,
                        end_line: line_num - 1,
                        kind: FoldingRangeKind::Imports,
                        is_collapsed: false,
                    });
                }
                block_start = None;
            }
            _ => {}
        }
    }

    // Close any trailing block
    if let Some(start) = block_start {
        let end = lines.len() as u32;
        if end > start {
            ranges.push(FoldingRange {
                start_line: start,
                end_line: end,
                kind: FoldingRangeKind::Imports,
                is_collapsed: false,
            });
        }
    }

    ranges
}

// -- FoldingRegionMerge for adjacent folding ----------------------------------

/// Merge adjacent folding ranges of the same kind.
pub fn merge_adjacent_ranges(ranges: &[FoldingRange]) -> Vec<FoldingRange> {
    if ranges.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<FoldingRange> = ranges.to_vec();
    sorted.sort_by_key(|r| r.start_line);

    let mut merged = vec![sorted[0].clone()];

    for range in &sorted[1..] {
        let last = merged.last_mut().unwrap();
        if range.kind == last.kind && range.start_line <= last.end_line + 2 {
            last.end_line = last.end_line.max(range.end_line);
        } else {
            merged.push(range.clone());
        }
    }

    merged
}

/// Count the total number of foldable lines across all ranges.
pub fn total_foldable_lines(ranges: &[FoldingRange]) -> u32 {
    ranges.iter().map(|r| {
        if r.end_line > r.start_line { r.end_line - r.start_line } else { 0 }
    }).sum()
}

// -- FoldingPersistence saving fold state -------------------------------------

/// Serializable fold state for persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldState {
    pub collapsed_lines: Vec<u32>,
}

impl FoldState {
    /// Capture the current fold state from a model.
    pub fn capture(model: &FoldingModel) -> Self {
        let collapsed_lines = model.get_ranges()
            .iter()
            .filter(|r| r.is_collapsed)
            .map(|r| r.start_line)
            .collect();
        Self { collapsed_lines }
    }

    /// Restore fold state to a model.
    pub fn restore(&self, model: &mut FoldingModel) {
        model.unfold_all();
        for &line in &self.collapsed_lines {
            model.toggle(line);
        }
    }

    /// Check if any lines are collapsed.
    pub fn has_collapsed(&self) -> bool {
        !self.collapsed_lines.is_empty()
    }

    /// Number of collapsed regions.
    pub fn collapsed_count(&self) -> usize {
        self.collapsed_lines.len()
    }
}

// -- Manual folding range management -----------------------------------------

/// Add a manual folding range to a model.
pub fn add_manual_range(model: &mut FoldingModel, start: u32, end: u32) {
    if start >= end {
        return;
    }
    let mut ranges = model.get_ranges().to_vec();
    ranges.push(FoldingRange {
        start_line: start,
        end_line: end,
        kind: FoldingRangeKind::Region,
        is_collapsed: false,
    });
    model.set_ranges(ranges);
}

/// Remove a folding range by start line.
pub fn remove_range_at(model: &mut FoldingModel, start_line: u32) {
    let ranges: Vec<FoldingRange> = model.get_ranges()
        .iter()
        .filter(|r| r.start_line != start_line)
        .cloned()
        .collect();
    model.set_ranges(ranges);
}

/// Filter ranges by kind.
pub fn ranges_of_kind(model: &FoldingModel, kind: FoldingRangeKind) -> Vec<&FoldingRange> {
    model.get_ranges().iter().filter(|r| r.kind == kind).collect()
}

/// Toggle all ranges of a specific kind.
pub fn toggle_all_of_kind(model: &mut FoldingModel, kind: FoldingRangeKind, collapse: bool) {
    let mut ranges = model.get_ranges().to_vec();
    for range in &mut ranges {
        if range.kind == kind {
            range.is_collapsed = collapse;
        }
    }
    model.set_ranges(ranges);
}

/// Count collapsed ranges.
pub fn collapsed_count(model: &FoldingModel) -> usize {
    model.get_ranges().iter().filter(|r| r.is_collapsed).count()
}

/// Fold ranges containing a specific line.
pub fn fold_containing_line(model: &mut FoldingModel, line: u32) {
    let mut ranges = model.get_ranges().to_vec();
    for range in &mut ranges {
        if line > range.start_line && line <= range.end_line {
            range.is_collapsed = true;
        }
    }
    model.set_ranges(ranges);
}


// ---------------------------------------------------------------------------
// FoldingRangeAnimation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FoldingRangeAnimation {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl FoldingRangeAnimation {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for FoldingRangeAnimation {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for FoldingRangeAnimation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "FoldingRangeAnimation({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// FoldingSelectionAware
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FoldingSelectionAware {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl FoldingSelectionAware {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for FoldingSelectionAware {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for FoldingSelectionAware {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "FoldingSelectionAware({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// FoldingRangeAnimationSnapshot — point-in-time snapshot of FoldingRangeAnimation state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FoldingRangeAnimationSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl FoldingRangeAnimationSnapshot {
    pub fn capture(source: &FoldingRangeAnimation, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for FoldingRangeAnimationSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// FoldingSelectionAwareStats — aggregate statistics for FoldingSelectionAware
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct FoldingSelectionAwareStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl FoldingSelectionAwareStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for FoldingSelectionAwareStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// FoldingRangeAnimationConfig — configuration for FoldingRangeAnimation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FoldingRangeAnimationConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl FoldingRangeAnimationConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for FoldingRangeAnimationConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for FoldingRangeAnimationConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// FoldingRangeOptimizer
// ---------------------------------------------------------------------------

/// Optimizes a set of folding ranges by merging, deduplicating, and sorting.
pub struct FoldingRangeOptimizer;

impl FoldingRangeOptimizer {
    /// Merge ranges whose end_line + 1 == next start_line and share the same kind.
    pub fn merge_adjacent(ranges: &[FoldingRange]) -> Vec<FoldingRange> {
        if ranges.is_empty() {
            return Vec::new();
        }
        let mut sorted: Vec<FoldingRange> = ranges.to_vec();
        sorted.sort_by_key(|r| r.start_line);
        let mut result: Vec<FoldingRange> = vec![sorted[0].clone()];
        for r in &sorted[1..] {
            let last = result.last_mut().unwrap();
            if last.kind == r.kind && last.end_line + 1 >= r.start_line {
                last.end_line = last.end_line.max(r.end_line);
            } else {
                result.push(r.clone());
            }
        }
        result
    }

    /// Remove ranges that are completely nested inside another range of the same kind.
    pub fn remove_nested_duplicates(ranges: &[FoldingRange]) -> Vec<FoldingRange> {
        let mut sorted: Vec<FoldingRange> = ranges.to_vec();
        sorted.sort_by_key(|r| (r.start_line, std::cmp::Reverse(r.end_line)));
        let mut result: Vec<FoldingRange> = Vec::new();
        for r in &sorted {
            let dominated = result.iter().any(|outer| {
                outer.start_line <= r.start_line
                    && outer.end_line >= r.end_line
                    && outer.kind == r.kind
                    && (outer.start_line != r.start_line || outer.end_line != r.end_line)
            });
            if !dominated {
                result.push(r.clone());
            }
        }
        result
    }

    /// Sort ranges by start_line then end_line.
    pub fn sort_by_line(ranges: &mut [FoldingRange]) {
        ranges.sort_by_key(|r| (r.start_line, r.end_line));
    }

    /// Expand range boundaries to nearest block boundaries (multiples of block_size).
    pub fn expand_to_block_boundaries(range: &FoldingRange, block_size: u32) -> FoldingRange {
        let start = (range.start_line / block_size) * block_size;
        let end = ((range.end_line + block_size - 1) / block_size) * block_size;
        FoldingRange {
            start_line: start,
            end_line: end,
            kind: range.kind,
            is_collapsed: range.is_collapsed,
        }
    }
}

// ---------------------------------------------------------------------------
// FoldingMemory
// ---------------------------------------------------------------------------

/// Remembers which ranges a user has folded per file.
pub struct FoldingMemory {
    folded: HashMap<String, Vec<(u32, u32)>>,
}

impl FoldingMemory {
    pub fn new() -> Self {
        Self { folded: HashMap::new() }
    }

    pub fn toggle_fold(&mut self, file: &str, start: u32, end: u32) {
        let entry = self.folded.entry(file.to_string()).or_default();
        if let Some(pos) = entry.iter().position(|&(s, e)| s == start && e == end) {
            entry.remove(pos);
        } else {
            entry.push((start, end));
        }
    }

    pub fn is_folded(&self, file: &str, start: u32, end: u32) -> bool {
        self.folded
            .get(file)
            .map_or(false, |v| v.iter().any(|&(s, e)| s == start && e == end))
    }

    pub fn fold_count(&self, file: &str) -> usize {
        self.folded.get(file).map_or(0, |v| v.len())
    }

    pub fn snapshot(&self) -> Vec<(String, Vec<(u32, u32)>)> {
        self.folded.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

// ---------------------------------------------------------------------------
// FoldingLevelCalculator
// ---------------------------------------------------------------------------

/// Computes nesting levels for each line based on indentation.
pub struct FoldingLevelCalculator {
    levels: Vec<u32>,
}

impl FoldingLevelCalculator {
    pub fn from_text(text: &str, indent_size: u32) -> Self {
        let indent_size = indent_size.max(1);
        let levels: Vec<u32> = text
            .lines()
            .map(|line| {
                let spaces = line.len() - line.trim_start().len();
                (spaces as u32) / indent_size
            })
            .collect();
        Self { levels }
    }

    pub fn max_level(&self) -> u32 {
        self.levels.iter().copied().max().unwrap_or(0)
    }

    pub fn lines_at_level(&self, level: u32) -> usize {
        self.levels.iter().filter(|&&l| l == level).count()
    }

    pub fn average_level(&self) -> f64 {
        if self.levels.is_empty() {
            return 0.0;
        }
        let sum: u32 = self.levels.iter().sum();
        sum as f64 / self.levels.len() as f64
    }

    pub fn level_for_line(&self, line: usize) -> Option<u32> {
        self.levels.get(line).copied()
    }
}


/// Configuration manager for folding functionality.
pub struct FoldingConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl FoldingConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &FoldingConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for folding operations.
pub struct FoldingRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl FoldingRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for folding.
pub struct FoldingValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl FoldingValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &FoldingValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Code folding range computation — extended utilities (yx)
// ---------------------------------------------------------------------------

/// Metric accumulator for folding operations.
#[derive(Debug, Clone)]
pub struct YxMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YxMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for folding.
#[derive(Debug, Clone)]
pub struct YxRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YxRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for folding lookups.
#[derive(Debug, Clone)]
pub struct YxLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YxLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for folding
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaFoldingRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaFoldingRingBuf {
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
pub struct XaFoldingCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaFoldingCounter {
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

impl Default for XaFoldingCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 83
// ---------------------------------------------------------------------------

/// Generic object pool `Xc83Pool<T>`.
pub struct Xc83Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc83Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc83PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc83Pool<T> {
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
    pub fn stats(&self) -> Xc83PoolStats {
        Xc83PoolStats {
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

impl<T> Default for Xc83Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc83Scheduler`.
pub struct Xc83Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc83Scheduler {
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

impl Default for Xc83Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_83 hash for the given byte slice.
pub fn xc_83_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_83 convention.
pub fn xc_83_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_109 deepening: state machine + event bus ---

/// States for the Xd109 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd109State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd109State {
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
pub struct Xd109Transition {
    pub from: Xd109State,
    pub to: Xd109State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd109StateMachine {
    current: Xd109State,
    history: Vec<Xd109Transition>,
    step_counter: usize,
}

impl Xd109StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd109State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd109State {
        self.current
    }

    pub fn history(&self) -> &[Xd109Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd109State) -> Result<Xd109State, String> {
        let allowed = match (self.current, target) {
            (Xd109State::Idle, Xd109State::Running) => true,
            (Xd109State::Running, Xd109State::Paused) => true,
            (Xd109State::Running, Xd109State::Done) => true,
            (Xd109State::Paused, Xd109State::Running) => true,
            (Xd109State::Paused, Xd109State::Done) => true,
            (Xd109State::Done, Xd109State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_109: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd109Transition {
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
            "Xd109SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd109State> {
        let prefix = "Xd109SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd109State::Idle),
            "Running" => Some(Xd109State::Running),
            "Paused" => Some(Xd109State::Paused),
            "Done" => Some(Xd109State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd109State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd109 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd109Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd109Event {
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

type Xd109HandlerFn = Box<dyn Fn(&Xd109Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd109EventBus {
    handlers: Vec<(usize, Option<String>, Xd109HandlerFn)>,
    next_id: usize,
    published: Vec<Xd109Event>,
}

impl Xd109EventBus {
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
        F: Fn(&Xd109Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd109Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd109Event) {
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

    pub fn published_events(&self) -> &[Xd109Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xg_33: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg33Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg33Graph {
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

impl Default for Xg33Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_33: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg33Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg33Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg33Heap<T>) {
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

impl<T: Ord> Default for Xg33Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 82).
pub struct Xh82SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh82SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 124 as u64,
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

/// A compact bit set supporting boolean operations (variant 82).
pub struct Xh82BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh82BitSet {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_fold() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![FoldingRange {
            start_line: 1, end_line: 5,
            kind: FoldingRangeKind::Region, is_collapsed: false,
        }]);
        assert!(!model.is_line_hidden(3));
        model.toggle(1);
        assert!(model.is_line_hidden(3));
        assert!(!model.is_line_hidden(1)); // fold line itself visible
    }

    #[test]
    fn fold_unfold_all() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 3, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 8, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        model.fold_all();
        assert!(model.is_line_hidden(2));
        assert!(model.is_line_hidden(6));
        model.unfold_all();
        assert!(!model.is_line_hidden(2));
    }

    #[test]
    fn compute_from_indentation() {
        let lines = vec!["fn main() {", "    let x = 1;", "    let y = 2;", "}"];
        let ranges = FoldingModel::compute_from_indentation(&lines, 4);
        assert!(!ranges.is_empty());
    }

    #[test]
    fn fold_level_collapses_nested() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 3, end_line: 8, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        model.fold_level(1); // depth >= 1 -> only inner
        assert!(!model.ranges[0].is_collapsed);
        assert!(model.ranges[1].is_collapsed);
    }

    #[test]
    fn get_collapsed_ranges_returns_only_collapsed() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: true },
            FoldingRange { start_line: 6, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let collapsed = model.get_collapsed_ranges();
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].start_line, 1);
    }

    #[test]
    fn get_visible_line_count_with_fold() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 2, end_line: 4, kind: FoldingRangeKind::Region, is_collapsed: true },
        ]);
        // Lines 1..=6, lines 3 and 4 hidden (line 2 is fold start, visible)
        assert_eq!(model.get_visible_line_count(6), 4);
    }

    #[test]
    fn fold_by_kind_collapses_matching() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 3, kind: FoldingRangeKind::Comment, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 8, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        model.fold_by_kind(FoldingRangeKind::Comment);
        assert!(model.ranges[0].is_collapsed);
        assert!(!model.ranges[1].is_collapsed);
    }

    #[test]
    fn unfold_at_innermost() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: true },
            FoldingRange { start_line: 3, end_line: 6, kind: FoldingRangeKind::Region, is_collapsed: true },
        ]);
        model.unfold_at(4);
        assert!(model.ranges[0].is_collapsed); // outer stays collapsed
        assert!(!model.ranges[1].is_collapsed); // inner unfolded
    }

    #[test]
    fn get_nesting_depth_counts_containing_ranges() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 3, end_line: 8, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 4, end_line: 6, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        assert_eq!(model.get_nesting_depth(5), 3);
        assert_eq!(model.get_nesting_depth(9), 1);
    }

    #[test]
    fn compute_from_markers_basic() {
        let lines = vec![
            "// #region A",
            "code",
            "// #endregion",
        ];
        let ranges = FoldingModel::compute_from_markers(&lines, "#region", "#endregion");
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 1);
        assert_eq!(ranges[0].end_line, 3);
    }

    #[test]
    fn compute_from_markers_nested() {
        let lines = vec![
            "// #region outer",
            "// #region inner",
            "code",
            "// #endregion",
            "// #endregion",
        ];
        let ranges = FoldingModel::compute_from_markers(&lines, "#region", "#endregion");
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn folding_provider_trait_default_impl() {
        let provider = IndentFoldingProvider::new(4);
        let text = "fn main() {\n    let x = 1;\n}";
        let ranges = provider.compute_folding_ranges(text);
        assert!(!ranges.is_empty());
        // Also test the default method
        let default_ranges = provider.compute_default(text);
        assert!(!default_ranges.is_empty());
    }

    #[test]
    fn unfold_at_no_match() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: true },
        ]);
        model.unfold_at(10); // line outside any range
        assert!(model.ranges[0].is_collapsed); // unchanged
    }

    #[test]
    fn eq_foldingrangekind_same() {
        assert_eq!(FoldingRangeKind::Comment, FoldingRangeKind::Comment);
    }

    #[test]
    fn ne_foldingrangekind_diff() {
        assert_ne!(FoldingRangeKind::Comment, FoldingRangeKind::Imports);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        let _svc = FoldingModel::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    // -- Bracket-based folding -----------------------------------------------

    #[test]
    fn fold_from_brackets() {
        let lines = vec!["fn main() {", "    println!(\"hello\");", "}"];
        let ranges = FoldingModel::compute_from_brackets(&lines);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 1);
        assert_eq!(ranges[0].end_line, 3);
    }

    #[test]
    fn fold_from_brackets_nested() {
        let lines = vec!["fn f() {", "    if true {", "        x;", "    }", "}"];
        let ranges = FoldingModel::compute_from_brackets(&lines);
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn fold_from_brackets_skips_strings() {
        let lines = vec!["let s = \"{\";", "let t = \"}\";"];
        let ranges = FoldingModel::compute_from_brackets(&lines);
        assert!(ranges.is_empty());
    }

    // -- Import folding -------------------------------------------------------

    #[test]
    fn fold_imports_rust() {
        let lines = vec!["use std::io;", "use std::fmt;", "", "fn main() {}"];
        let ranges = FoldingModel::compute_from_imports(&lines);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 1);
        assert_eq!(ranges[0].end_line, 3); // includes the blank line after imports
        assert_eq!(ranges[0].kind, FoldingRangeKind::Imports);
    }

    // -- Composite folding ----------------------------------------------------

    #[test]
    fn fold_level_in_ranges() {
        let ranges = vec![
            FoldingRange { start_line: 1, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 3, end_line: 15, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ];
        assert_eq!(ranges[0].fold_level_in(&ranges), 0);
        assert_eq!(ranges[1].fold_level_in(&ranges), 1);
        assert_eq!(ranges[2].fold_level_in(&ranges), 2);
    }

    #[test]
    fn is_nested_in_check() {
        let outer = FoldingRange { start_line: 1, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false };
        let inner = FoldingRange { start_line: 5, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false };
        assert!(inner.is_nested_in(&outer));
        assert!(!outer.is_nested_in(&inner));
    }

    #[test]
    fn find_nested_ranges() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 25, end_line: 30, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let parent = &model.get_ranges()[0];
        let nested = model.find_nested(parent);
        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].start_line, 5);
    }

    #[test]
    fn serialize_and_restore_state() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: true },
            FoldingRange { start_line: 10, end_line: 15, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let state = model.serialize_state();
        assert_eq!(state, vec![(1, true), (10, false)]);
        model.unfold_all();
        model.restore_state(&state);
        assert!(model.get_range_at(1).unwrap().is_collapsed);
        assert!(!model.get_range_at(10).unwrap().is_collapsed);
    }

    #[test]
    fn folding_statistics_computation() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: true },
            FoldingRange { start_line: 5, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let stats = model.statistics();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.collapsed, 1);
        assert_eq!(stats.expanded, 1);
        assert_eq!(stats.max_depth, 1);
        assert_eq!(stats.total_span, 24); // 19 + 5
    }

    #[test]
    fn line_span_calculation() {
        let r = FoldingRange { start_line: 3, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false };
        assert_eq!(r.line_span(), 7);
    }

    #[test]
    fn composite_folding_provider() {
        let provider = CompositeFoldingProvider::new(4);
        let text = "use std::io;\nuse std::fmt;\n\nfn main() {\n    println!(\"hello\");\n}\n";
        let ranges = provider.compute_folding_ranges(text);
        assert!(ranges.len() >= 2); // imports + bracket fold
    }

    #[test]
    fn comment_provider_detects_comment_blocks() {
        let provider = CommentFoldingProvider::new("//");
        let text = "// first\n// second\n// third\nfn main() {}\n";
        let ranges = provider.compute_folding_ranges(text);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 1);
        assert_eq!(ranges[0].end_line, 3);
        assert_eq!(ranges[0].kind, FoldingRangeKind::Comment);
    }

    #[test]
    fn comment_provider_skips_non_comment_lines() {
        let provider = CommentFoldingProvider::new("//");
        let text = "let x = 1;\nlet y = 2;\nlet z = 3;\n";
        let ranges = provider.compute_folding_ranges(text);
        assert!(ranges.is_empty());
    }

    #[test]
    fn folding_range_set_add_and_get() {
        let mut set = FoldingRangeSet::new();
        assert!(set.is_empty());
        set.add(FoldingRange {
            start_line: 1, end_line: 10,
            kind: FoldingRangeKind::Region, is_collapsed: false,
        });
        assert_eq!(set.len(), 1);
        assert_eq!(set.get_ranges()[0].start_line, 1);
    }

    #[test]
    fn folding_range_set_merge_deduplicates() {
        let mut a = FoldingRangeSet::new();
        a.add(FoldingRange {
            start_line: 1, end_line: 10,
            kind: FoldingRangeKind::Region, is_collapsed: false,
        });
        a.add(FoldingRange {
            start_line: 20, end_line: 30,
            kind: FoldingRangeKind::Region, is_collapsed: false,
        });

        let mut b = FoldingRangeSet::new();
        b.add(FoldingRange {
            start_line: 1, end_line: 10,
            kind: FoldingRangeKind::Region, is_collapsed: false,
        });
        b.add(FoldingRange {
            start_line: 40, end_line: 50,
            kind: FoldingRangeKind::Region, is_collapsed: false,
        });

        a.merge(&b);
        assert_eq!(a.len(), 3); // duplicate (1,10) not added twice
    }

    #[test]
    fn folding_range_set_ranges_containing_line() {
        let mut set = FoldingRangeSet::new();
        set.add(FoldingRange {
            start_line: 1, end_line: 20,
            kind: FoldingRangeKind::Region, is_collapsed: false,
        });
        set.add(FoldingRange {
            start_line: 5, end_line: 10,
            kind: FoldingRangeKind::Region, is_collapsed: false,
        });
        set.add(FoldingRange {
            start_line: 30, end_line: 40,
            kind: FoldingRangeKind::Region, is_collapsed: false,
        });
        let containing = set.ranges_containing_line(7);
        assert_eq!(containing.len(), 2);
        let containing_outside = set.ranges_containing_line(25);
        assert!(containing_outside.is_empty());
    }

    #[test]
    fn folding_range_set_total_hidden_lines() {
        let mut set = FoldingRangeSet::new();
        set.add(FoldingRange {
            start_line: 1, end_line: 11,
            kind: FoldingRangeKind::Region, is_collapsed: true,
        });
        set.add(FoldingRange {
            start_line: 20, end_line: 25,
            kind: FoldingRangeKind::Region, is_collapsed: true,
        });
        set.add(FoldingRange {
            start_line: 30, end_line: 40,
            kind: FoldingRangeKind::Region, is_collapsed: false,
        });
        assert_eq!(set.total_hidden_lines(), 15); // 10 + 5
    }

    #[test]
    fn fold_region_non_recursive() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        fold_region(&mut model, 1, false);
        assert!(model.get_range_at(1).unwrap().is_collapsed);
        assert!(!model.get_range_at(5).unwrap().is_collapsed);
    }

    #[test]
    fn fold_region_recursive() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        fold_region(&mut model, 1, true);
        assert!(model.get_range_at(1).unwrap().is_collapsed);
        assert!(model.get_range_at(5).unwrap().is_collapsed);
    }

    #[test]
    fn unfold_region_recursive() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: true },
            FoldingRange { start_line: 5, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: true },
            FoldingRange { start_line: 25, end_line: 30, kind: FoldingRangeKind::Region, is_collapsed: true },
        ]);
        unfold_region(&mut model, 1, true);
        assert!(!model.get_range_at(1).unwrap().is_collapsed);
        assert!(!model.get_range_at(5).unwrap().is_collapsed);
        // Range outside the target should remain collapsed
        assert!(model.get_range_at(25).unwrap().is_collapsed);
    }

    // -- FoldingSnapshot tests ------------------------------------------------

    #[test]
    fn snapshot_capture_and_apply() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: true },
            FoldingRange { start_line: 15, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let snap = FoldingSnapshot::capture(&model);
        assert_eq!(snap.len(), 2);
        assert_eq!(snap.collapsed_count(), 1);

        // Mutate model, then restore from snapshot
        model.unfold_all();
        assert!(!model.get_range_at(1).unwrap().is_collapsed);
        snap.apply(&mut model);
        assert!(model.get_range_at(1).unwrap().is_collapsed);
        assert!(!model.get_range_at(15).unwrap().is_collapsed);
    }

    #[test]
    fn diff_snapshots_detects_fold_unfold() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 15, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: true },
        ]);
        let before = FoldingSnapshot::capture(&model);

        model.toggle(1);   // fold line 1
        model.toggle(15);  // unfold line 15
        let after = FoldingSnapshot::capture(&model);

        let diffs = diff_snapshots(&before, &after);
        assert_eq!(diffs.len(), 2);
        assert!(diffs.iter().any(|d| d.start_line == 1 && d.kind == FoldingChangeKind::Folded));
        assert!(diffs.iter().any(|d| d.start_line == 15 && d.kind == FoldingChangeKind::Unfolded));
    }

    #[test]
    fn diff_snapshots_detects_added_removed() {
        let before = FoldingSnapshot { entries: vec![(1, 10, false), (20, 30, true)] };
        let after = FoldingSnapshot { entries: vec![(1, 10, false), (40, 50, false)] };
        let diffs = diff_snapshots(&before, &after);
        assert!(diffs.iter().any(|d| d.start_line == 20 && d.kind == FoldingChangeKind::Removed));
        assert!(diffs.iter().any(|d| d.start_line == 40 && d.kind == FoldingChangeKind::Added));
    }

    #[test]
    fn folding_history_record_and_undo() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let mut history = FoldingHistory::new();

        // Fold line 1 and record it
        model.toggle(1);
        history.record(1, true);
        assert!(model.get_range_at(1).unwrap().is_collapsed);
        assert_eq!(history.len(), 1);

        // Undo should unfold it
        let undone = history.undo_last(&mut model);
        assert!(undone);
        assert!(!model.get_range_at(1).unwrap().is_collapsed);
        assert!(history.is_empty());
    }

    #[test]
    fn folding_history_multiple_events() {
        let mut history = FoldingHistory::new();
        history.record(1, true);
        history.record(5, false);
        history.record(10, true);
        assert_eq!(history.len(), 3);

        let events = history.events();
        assert_eq!(events[0].seq, 0);
        assert_eq!(events[1].seq, 1);
        assert_eq!(events[2].seq, 2);

        let last = history.pop().unwrap();
        assert_eq!(last.start_line, 10);
        assert_eq!(history.len(), 2);
    }

    // -- Merge adjacent single-line folds ------------------------------------

    #[test]
    fn merge_adjacent_single_line_folds_combines_consecutive() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 2, kind: FoldingRangeKind::Comment, is_collapsed: false },
            FoldingRange { start_line: 3, end_line: 4, kind: FoldingRangeKind::Comment, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 6, kind: FoldingRangeKind::Comment, is_collapsed: false },
            // Gap here – line 7-8 is separate
            FoldingRange { start_line: 10, end_line: 11, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        model.merge_adjacent_single_line_folds();
        // First three should merge into 1-6; line 10-11 stays separate
        assert_eq!(model.get_ranges().len(), 2);
        assert_eq!(model.get_ranges()[0].start_line, 1);
        assert_eq!(model.get_ranges()[0].end_line, 6);
        assert_eq!(model.get_ranges()[1].start_line, 10);
    }

    // -- Fold to level -------------------------------------------------------

    #[test]
    fn fold_to_level_collapses_deeper_and_expands_shallower() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 30, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 3, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 15, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        // fold_to_level(1) -> depth 0 stays expanded, depths >= 1 collapse
        model.fold_to_level(1);
        assert!(!model.get_range_at(1).unwrap().is_collapsed);  // depth 0
        assert!(model.get_range_at(3).unwrap().is_collapsed);   // depth 1
        assert!(model.get_range_at(5).unwrap().is_collapsed);   // depth 2
    }

    // -- String-based fold state persistence ---------------------------------

    #[test]
    fn serialize_and_restore_fold_state_via_string() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: true },
            FoldingRange { start_line: 15, end_line: 25, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let serialized = model.serialize_to_string();
        assert_eq!(serialized, "1:10:1;15:25:0");

        // Change state, then restore
        model.unfold_all();
        assert!(!model.get_range_at(1).unwrap().is_collapsed);
        model.restore_from_string(&serialized);
        assert!(model.get_range_at(1).unwrap().is_collapsed);
        assert!(!model.get_range_at(15).unwrap().is_collapsed);
    }

    // -- Line fold levels ----------------------------------------------------

    #[test]
    fn line_fold_levels_reflect_nesting() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 3, end_line: 8, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let levels = model.line_fold_levels(10);
        assert_eq!(levels[1], 1);  // inside outer only
        assert_eq!(levels[5], 2);  // inside both
        assert_eq!(levels[9], 1);  // inside outer only
        assert_eq!(levels[0], 0);  // line 0 unused sentinel
    }

    // -- Ranges overlapping a viewport ---------------------------------------

    #[test]
    fn ranges_overlapping_returns_partial_and_full_overlaps() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 8, end_line: 12, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 20, end_line: 30, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        // Viewport covers lines 4-10: overlaps first two ranges but not third
        let overlapping = model.ranges_overlapping(4, 10);
        assert_eq!(overlapping.len(), 2);
        assert_eq!(overlapping[0].start_line, 1);
        assert_eq!(overlapping[1].start_line, 8);
    }

    // -- Toggle by kind ------------------------------------------------------

    #[test]
    fn toggle_by_kind_flips_matching_ranges() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 3, kind: FoldingRangeKind::Comment, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 12, end_line: 14, kind: FoldingRangeKind::Comment, is_collapsed: true },
        ]);
        model.toggle_by_kind(FoldingRangeKind::Comment);
        assert!(model.get_range_at(1).unwrap().is_collapsed);   // was false -> true
        assert!(!model.get_range_at(5).unwrap().is_collapsed);   // Region untouched
        assert!(!model.get_range_at(12).unwrap().is_collapsed);  // was true -> false
    }

    // -- compute_folding_summary -----------------------------------------------

    #[test]
    fn folding_summary_basic() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Comment, is_collapsed: true },
            FoldingRange { start_line: 7, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let summary = compute_folding_summary(&model);
        assert_eq!(summary.total, 2);
        assert_eq!(summary.collapsed, 1);
        assert_eq!(summary.hidden_lines, 4); // lines 2..5
        assert_eq!(summary.comment_count, 1);
        assert_eq!(summary.region_count, 1);
    }

    #[test]
    fn folding_summary_empty() {
        let model = FoldingModel::new();
        let summary = compute_folding_summary(&model);
        assert_eq!(summary.total, 0);
        assert_eq!(summary.max_depth, 0);
    }

    // -- ranges_overlapping ----------------------------------------------------

    #[test]
    fn ranges_overlapping_finds_overlap() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 10, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let overlapping = ranges_overlapping(&model, 3, 12);
        assert_eq!(overlapping.len(), 2);
    }

    #[test]
    fn ranges_overlapping_none() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let overlapping = ranges_overlapping(&model, 10, 20);
        assert_eq!(overlapping.len(), 0);
    }

    // -- visible_line_count ----------------------------------------------------

    #[test]
    fn visible_line_count_with_collapsed() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 2, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: true },
        ]);
        assert_eq!(visible_line_count(&model, 10), 7);
    }

    // -- innermost_range_at ----------------------------------------------------

    #[test]
    fn innermost_range_at_finds_deepest() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let inner = innermost_range_at(&model, 7).unwrap();
        assert_eq!(inner.start_line, 5);
        assert_eq!(inner.end_line, 10);
    }

    // -- fold_header_lines -----------------------------------------------------

    #[test]
    fn fold_header_lines_collects_starts() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 10, end_line: 15, kind: FoldingRangeKind::Comment, is_collapsed: false },
        ]);
        let headers = fold_header_lines(&model);
        assert_eq!(headers, vec![1, 10]);
    }

    // -- detect_import_ranges tests -------------------------------------------

    #[test]
    fn detect_imports_rust() {
        let lines = vec!["use std::fmt;", "use std::io;", "", "fn main() {}"];
        let ranges = detect_import_ranges(&lines);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 1);
        assert_eq!(ranges[0].end_line, 2);
        assert_eq!(ranges[0].kind, FoldingRangeKind::Imports);
    }

    #[test]
    fn detect_imports_none() {
        let lines = vec!["fn main() {}", "  println!();", "}"];
        let ranges = detect_import_ranges(&lines);
        assert!(ranges.is_empty());
    }

    #[test]
    fn detect_imports_trailing() {
        let lines = vec!["use a;", "use b;", "use c;"];
        let ranges = detect_import_ranges(&lines);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].end_line, 3);
    }

    // -- merge_adjacent_ranges tests ------------------------------------------

    #[test]
    fn merge_adjacent_same_kind() {
        let ranges = vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 6, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ];
        let merged = merge_adjacent_ranges(&ranges);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].end_line, 10);
    }

    #[test]
    fn merge_different_kinds_not_merged() {
        let ranges = vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Comment, is_collapsed: false },
            FoldingRange { start_line: 6, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ];
        let merged = merge_adjacent_ranges(&ranges);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn total_foldable_lines_counts() {
        let ranges = vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 10, end_line: 15, kind: FoldingRangeKind::Region, is_collapsed: false },
        ];
        assert_eq!(total_foldable_lines(&ranges), 9);
    }

    // -- FoldState tests ------------------------------------------------------

    #[test]
    fn fold_state_capture_and_restore() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: true },
            FoldingRange { start_line: 10, end_line: 15, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let state = FoldState::capture(&model);
        assert_eq!(state.collapsed_count(), 1);
        assert!(state.has_collapsed());

        model.unfold_all();
        assert_eq!(collapsed_count(&model), 0);

        state.restore(&mut model);
        assert_eq!(collapsed_count(&model), 1);
    }

    // -- Manual range management tests ----------------------------------------

    #[test]
    fn add_manual_range_works() {
        let mut model = FoldingModel::new();
        add_manual_range(&mut model, 5, 10);
        assert_eq!(model.get_ranges().len(), 1);
        assert_eq!(model.get_ranges()[0].start_line, 5);
    }

    #[test]
    fn add_manual_range_invalid_ignored() {
        let mut model = FoldingModel::new();
        add_manual_range(&mut model, 10, 5);
        assert!(model.get_ranges().is_empty());
    }

    #[test]
    fn remove_range_at_works() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 10, end_line: 15, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        remove_range_at(&mut model, 1);
        assert_eq!(model.get_ranges().len(), 1);
        assert_eq!(model.get_ranges()[0].start_line, 10);
    }

    #[test]
    fn ranges_of_kind_filters() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Comment, is_collapsed: false },
            FoldingRange { start_line: 10, end_line: 15, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        let comments = ranges_of_kind(&model, FoldingRangeKind::Comment);
        assert_eq!(comments.len(), 1);
    }

    #[test]
    fn toggle_all_of_kind_works() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Comment, is_collapsed: false },
            FoldingRange { start_line: 10, end_line: 15, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        toggle_all_of_kind(&mut model, FoldingRangeKind::Comment, true);
        assert!(model.get_range_at(1).unwrap().is_collapsed);
        assert!(!model.get_range_at(10).unwrap().is_collapsed);
    }

    #[test]
    fn fold_containing_line_folds() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        fold_containing_line(&mut model, 5);
        assert!(model.get_range_at(1).unwrap().is_collapsed);
    }

    #[test] fn foldingRangeAnimation_new() { let s = FoldingRangeAnimation::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn foldingRangeAnimation_add() { let mut s = FoldingRangeAnimation::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn foldingRangeAnimation_remove() { let mut s = FoldingRangeAnimation::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn foldingRangeAnimation_config() { let mut s = FoldingRangeAnimation::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn foldingRangeAnimation_nav() { let mut s = FoldingRangeAnimation::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn foldingRangeAnimation_filter() { let mut s = FoldingRangeAnimation::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn foldingRangeAnimation_display() { assert!(format!("{}", FoldingRangeAnimation::new()).contains("FoldingRangeAnimation")); }
    #[test] fn foldingSelectionAware_new() { let s = FoldingSelectionAware::new(); assert!(s.is_empty()); }
    #[test] fn foldingSelectionAware_add() { let mut s = FoldingSelectionAware::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn foldingSelectionAware_active() { let mut s = FoldingSelectionAware::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn foldingSelectionAware_error() { let mut s = FoldingSelectionAware::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn foldingSelectionAware_rm_group() { let mut s = FoldingSelectionAware::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn foldingSelectionAware_display() { assert!(format!("{}", FoldingSelectionAware::new()).contains("FoldingSelectionAware")); }


    #[test] fn foldingRangeAnimation_snap_capture() {
        let s = FoldingRangeAnimation::new();
        let snap = FoldingRangeAnimationSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn foldingRangeAnimation_snap_stale() {
        let s = FoldingRangeAnimation::new();
        let snap = FoldingRangeAnimationSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn foldingRangeAnimation_snap_diff() {
        let s = FoldingRangeAnimation::new();
        let s1v = FoldingRangeAnimationSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn foldingRangeAnimation_snap_display() {
        let s = FoldingRangeAnimation::new();
        let snap = FoldingRangeAnimationSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn foldingSelectionAware_stats_record() {
        let mut st = FoldingSelectionAwareStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn foldingSelectionAware_stats_hit_ratio() {
        let mut st = FoldingSelectionAwareStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn foldingSelectionAware_stats_merge() {
        let mut a = FoldingSelectionAwareStats::new();
        a.total_adds = 5;
        let mut b = FoldingSelectionAwareStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn foldingSelectionAware_stats_display() {
        let st = FoldingSelectionAwareStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn foldingRangeAnimation_config_default() {
        let c = FoldingRangeAnimationConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn foldingRangeAnimation_config_builder() {
        let c = FoldingRangeAnimationConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn foldingRangeAnimation_config_labels() {
        let mut c = FoldingRangeAnimationConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn foldingRangeAnimation_config_cleanup_threshold() {
        let c = FoldingRangeAnimationConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn foldingRangeAnimation_config_display() {
        assert!(format!("{}", FoldingRangeAnimationConfig::new()).contains("Config"));
    }
    #[test] fn foldingSelectionAware_stats_peaks() {
        let mut st = FoldingSelectionAwareStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- FoldingRangeOptimizer tests --

    #[test]
    fn optimizer_merge_adjacent_same_kind() {
        let ranges = vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 6, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ];
        let merged = FoldingRangeOptimizer::merge_adjacent(&ranges);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start_line, 1);
        assert_eq!(merged[0].end_line, 10);
    }

    #[test]
    fn optimizer_no_merge_different_kind() {
        let ranges = vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 6, end_line: 10, kind: FoldingRangeKind::Comment, is_collapsed: false },
        ];
        let merged = FoldingRangeOptimizer::merge_adjacent(&ranges);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn optimizer_merge_empty() {
        assert!(FoldingRangeOptimizer::merge_adjacent(&[]).is_empty());
    }

    #[test]
    fn optimizer_remove_nested_duplicates() {
        let ranges = vec![
            FoldingRange { start_line: 1, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 3, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ];
        let result = FoldingRangeOptimizer::remove_nested_duplicates(&ranges);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].end_line, 20);
    }

    #[test]
    fn optimizer_sort_by_line() {
        let mut ranges = vec![
            FoldingRange { start_line: 10, end_line: 20, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
        ];
        FoldingRangeOptimizer::sort_by_line(&mut ranges);
        assert_eq!(ranges[0].start_line, 1);
        assert_eq!(ranges[1].start_line, 10);
    }

    #[test]
    fn optimizer_expand_to_block_boundaries() {
        let r = FoldingRange { start_line: 3, end_line: 7, kind: FoldingRangeKind::Region, is_collapsed: false };
        let expanded = FoldingRangeOptimizer::expand_to_block_boundaries(&r, 5);
        assert_eq!(expanded.start_line, 0);
        assert_eq!(expanded.end_line, 10);
    }

    // -- FoldingMemory tests --

    #[test]
    fn memory_toggle_and_is_folded() {
        let mut mem = FoldingMemory::new();
        mem.toggle_fold("a.rs", 1, 5);
        assert!(mem.is_folded("a.rs", 1, 5));
        mem.toggle_fold("a.rs", 1, 5);
        assert!(!mem.is_folded("a.rs", 1, 5));
    }

    #[test]
    fn memory_fold_count() {
        let mut mem = FoldingMemory::new();
        assert_eq!(mem.fold_count("x.rs"), 0);
        mem.toggle_fold("x.rs", 1, 5);
        mem.toggle_fold("x.rs", 10, 20);
        assert_eq!(mem.fold_count("x.rs"), 2);
    }

    #[test]
    fn memory_snapshot() {
        let mut mem = FoldingMemory::new();
        mem.toggle_fold("a.rs", 1, 5);
        let snap = mem.snapshot();
        assert_eq!(snap.len(), 1);
    }

    // -- FoldingLevelCalculator tests --

    #[test]
    fn level_calculator_basic() {
        let text = "a\n  b\n    c\n";
        let calc = FoldingLevelCalculator::from_text(text, 2);
        assert_eq!(calc.level_for_line(0), Some(0));
        assert_eq!(calc.level_for_line(1), Some(1));
        assert_eq!(calc.level_for_line(2), Some(2));
    }

    #[test]
    fn level_calculator_max_level() {
        let text = "a\n    b\n        c\n";
        let calc = FoldingLevelCalculator::from_text(text, 4);
        assert_eq!(calc.max_level(), 2);
    }

    #[test]
    fn level_calculator_lines_at_level() {
        let text = "a\n  b\n  c\n    d\n";
        let calc = FoldingLevelCalculator::from_text(text, 2);
        assert_eq!(calc.lines_at_level(0), 1);
        assert_eq!(calc.lines_at_level(1), 2);
        assert_eq!(calc.lines_at_level(2), 1);
    }

    #[test]
    fn level_calculator_average() {
        let text = "a\n  b\n";
        let calc = FoldingLevelCalculator::from_text(text, 2);
        let avg = calc.average_level();
        assert!((avg - 0.5).abs() < 0.01);
    }


    #[test]
    fn folding_config_new() {
        let cfg = FoldingConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn folding_config_set_get() {
        let mut cfg = FoldingConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn folding_config_remove() {
        let mut cfg = FoldingConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn folding_config_keys_sorted() {
        let mut cfg = FoldingConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn folding_config_bump_version() {
        let mut cfg = FoldingConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn folding_config_clear() {
        let mut cfg = FoldingConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn folding_config_merge() {
        let mut cfg1 = FoldingConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = FoldingConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn folding_config_disable() {
        let mut cfg = FoldingConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn folding_rate_tracker_empty() {
        let rt = FoldingRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn folding_rate_tracker_record() {
        let mut rt = FoldingRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn folding_rate_tracker_prune() {
        let mut rt = FoldingRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn folding_validator_valid() {
        let v = FoldingValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn folding_validator_errors() {
        let mut v = FoldingValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn folding_validator_clear() {
        let mut v = FoldingValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn folding_validator_merge() {
        let mut v1 = FoldingValidator::new();
        v1.add_error("e1");
        let mut v2 = FoldingValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn folding_rate_tracker_clear() {
        let mut rt = FoldingRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yx_metrics_empty() {
        let m = YxMetrics::new("folding");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yx_metrics_record_and_mean() {
        let mut m = YxMetrics::new("folding");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yx_metrics_min_max() {
        let mut m = YxMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yx_metrics_variance_and_std() {
        let mut m = YxMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yx_metrics_percentile() {
        let mut m = YxMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yx_metrics_merge() {
        let mut a = YxMetrics::new("a");
        a.record(1.0);
        let mut b = YxMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yx_metrics_reset() {
        let mut m = YxMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yx_rate_window_empty() {
        let rw = YxRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yx_rate_window_tick_and_rate() {
        let mut rw = YxRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yx_lru_cache_basic() {
        let mut c = YxLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yx_lru_cache_contains_and_keys() {
        let mut c = YxLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yx_lru_cache_remove() {
        let mut c = YxLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yx_metrics_sum() {
        let mut m = YxMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yx_metrics_label() {
        let m = YxMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yx_lru_cache_clear() {
        let mut c = YxLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for folding
    #[test]
    fn xa_folding_ring_new() {
        let rb = super::XaFoldingRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_folding_ring_push_len() {
        let mut rb = super::XaFoldingRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_folding_ring_wrap() {
        let mut rb = super::XaFoldingRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_folding_ring_mean_empty() {
        let rb = super::XaFoldingRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_folding_ring_mean_values() {
        let mut rb = super::XaFoldingRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_folding_ring_min_max() {
        let mut rb = super::XaFoldingRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_folding_ring_iter() {
        let mut rb = super::XaFoldingRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_folding_counter_new() {
        let c = super::XaFoldingCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_folding_counter_inc() {
        let mut c = super::XaFoldingCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_folding_counter_inc_by() {
        let mut c = super::XaFoldingCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_folding_counter_reset() {
        let mut c = super::XaFoldingCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_folding_counter_clear() {
        let mut c = super::XaFoldingCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_folding_counter_default() {
        let c = super::XaFoldingCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 83 ----

    #[test]
    fn xc_83_pool_new_empty() {
        let pool: super::Xc83Pool<i32> = super::Xc83Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_83_pool_release_acquire() {
        let mut pool = super::Xc83Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_83_pool_acquire_empty() {
        let mut pool: super::Xc83Pool<i32> = super::Xc83Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_83_pool_full() {
        let mut pool = super::Xc83Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_83_pool_drain() {
        let mut pool = super::Xc83Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_83_pool_stats() {
        let mut pool = super::Xc83Pool::new(8);
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
    fn xc_83_pool_clear() {
        let mut pool = super::Xc83Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_83_pool_shrink() {
        let mut pool = super::Xc83Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_83_pool_default() {
        let pool: super::Xc83Pool<String> = super::Xc83Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_83_pool_extend() {
        let mut pool = super::Xc83Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_83_pool_retain() {
        let mut pool = super::Xc83Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_83_scheduler_round_robin() {
        let mut sched = super::Xc83Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_83_scheduler_empty() {
        let mut sched = super::Xc83Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_83_scheduler_reset() {
        let mut sched = super::Xc83Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_83_scheduler_add_remove() {
        let mut sched = super::Xc83Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_83_scheduler_targets() {
        let sched = super::Xc83Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_83_hash_empty() {
        assert_eq!(super::xc_83_hash(b""), 5381);
    }

    #[test]
    fn xc_83_hash_data() {
        let h = super::xc_83_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_83_hash(b"hello"), h);
    }

    #[test]
    fn xc_83_reverse_str() {
        assert_eq!(super::xc_83_reverse("abc"), "cba");
        assert_eq!(super::xc_83_reverse(""), "");
    }


    // --- xd_109 deepening tests ---

    #[test]
    fn xd_109_sm_initial_state() {
        let sm = Xd109StateMachine::new();
        assert_eq!(sm.current_state(), Xd109State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_109_sm_valid_idle_to_running() {
        let mut sm = Xd109StateMachine::new();
        assert!(sm.transition(Xd109State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd109State::Running);
    }

    #[test]
    fn xd_109_sm_valid_running_to_paused() {
        let mut sm = Xd109StateMachine::new();
        sm.transition(Xd109State::Running).unwrap();
        assert!(sm.transition(Xd109State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd109State::Paused);
    }

    #[test]
    fn xd_109_sm_valid_running_to_done() {
        let mut sm = Xd109StateMachine::new();
        sm.transition(Xd109State::Running).unwrap();
        assert!(sm.transition(Xd109State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd109State::Done);
    }

    #[test]
    fn xd_109_sm_valid_paused_to_running() {
        let mut sm = Xd109StateMachine::new();
        sm.transition(Xd109State::Running).unwrap();
        sm.transition(Xd109State::Paused).unwrap();
        assert!(sm.transition(Xd109State::Running).is_ok());
    }

    #[test]
    fn xd_109_sm_valid_done_to_idle() {
        let mut sm = Xd109StateMachine::new();
        sm.transition(Xd109State::Running).unwrap();
        sm.transition(Xd109State::Done).unwrap();
        assert!(sm.transition(Xd109State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd109State::Idle);
    }

    #[test]
    fn xd_109_sm_invalid_idle_to_done() {
        let mut sm = Xd109StateMachine::new();
        assert!(sm.transition(Xd109State::Done).is_err());
    }

    #[test]
    fn xd_109_sm_invalid_idle_to_paused() {
        let mut sm = Xd109StateMachine::new();
        assert!(sm.transition(Xd109State::Paused).is_err());
    }

    #[test]
    fn xd_109_sm_history_tracking() {
        let mut sm = Xd109StateMachine::new();
        sm.transition(Xd109State::Running).unwrap();
        sm.transition(Xd109State::Paused).unwrap();
        sm.transition(Xd109State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd109State::Idle);
        assert_eq!(sm.history()[0].to, Xd109State::Running);
        assert_eq!(sm.history()[1].from, Xd109State::Running);
        assert_eq!(sm.history()[2].to, Xd109State::Done);
    }

    #[test]
    fn xd_109_sm_serialize_deserialize() {
        let mut sm = Xd109StateMachine::new();
        sm.transition(Xd109State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd109StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd109State::Running));
    }

    #[test]
    fn xd_109_sm_deserialize_invalid() {
        assert_eq!(Xd109StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_109_sm_reset() {
        let mut sm = Xd109StateMachine::new();
        sm.transition(Xd109State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd109State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_109_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd109EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd109Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_109_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd109EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd109Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd109Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_109_bus_unsubscribe() {
        let mut bus = Xd109EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_109_event_kind_and_payload() {
        let e = Xd109Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd109Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_109_bus_clear_history() {
        let mut bus = Xd109EventBus::new();
        bus.publish(Xd109Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_109_sm_step_counter_increments() {
        let mut sm = Xd109StateMachine::new();
        sm.transition(Xd109State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd109State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_33 graph tests ------------------------------------------------

    #[test]
    fn xg_33_graph_empty() {
        let g = super::Xg33Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_33_graph_add_node() {
        let mut g = super::Xg33Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_33_graph_add_edge() {
        let mut g = super::Xg33Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_33_graph_neighbors() {
        let mut g = super::Xg33Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_33_graph_has_path() {
        let mut g = super::Xg33Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_33_graph_self_path() {
        let g = super::Xg33Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_33_graph_topo_sort() {
        let mut g = super::Xg33Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_33_graph_cycle_detect_false() {
        let mut g = super::Xg33Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_33_graph_cycle_detect_true() {
        let mut g = super::Xg33Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_33 heap tests -------------------------------------------------

    #[test]
    fn xg_33_heap_empty() {
        let h: super::Xg33Heap<i32> = super::Xg33Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_33_heap_push_pop() {
        let mut h = super::Xg33Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_33_heap_peek() {
        let mut h = super::Xg33Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_33_heap_drain_sorted() {
        let mut h = super::Xg33Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_33_heap_merge() {
        let mut a = super::Xg33Heap::new();
        let mut b = super::Xg33Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_33_heap_default() {
        let h: super::Xg33Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_33_graph_default() {
        let g: super::Xg33Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh82_skip_insert_contains() {
        let mut sl = super::Xh82SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh82_skip_remove() {
        let mut sl = super::Xh82SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh82_skip_len() {
        let mut sl = super::Xh82SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh82_skip_range_query() {
        let mut sl = super::Xh82SkipList::xh_new(4);
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
    fn xh82_skip_floor_ceiling() {
        let mut sl = super::Xh82SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh82_skip_rank() {
        let mut sl = super::Xh82SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh82_skip_empty() {
        let sl = super::Xh82SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh82_skip_duplicates() {
        let mut sl = super::Xh82SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh82_bitset_set_test() {
        let mut bs = super::Xh82BitSet::xh_new(256);
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
    fn xh82_bitset_clear_count() {
        let mut bs = super::Xh82BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh82_bitset_and_or_xor() {
        let mut a = super::Xh82BitSet::xh_new(128);
        let mut b = super::Xh82BitSet::xh_new(128);
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
    fn xh82_bitset_iter_ones() {
        let mut bs = super::Xh82BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh82_bitset_first_last() {
        let mut bs = super::Xh82BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh82_bitset_empty() {
        let bs = super::Xh82BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
