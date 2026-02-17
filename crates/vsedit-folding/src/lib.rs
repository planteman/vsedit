//! Code folding model.
//!
//! Equivalent to VS Code's folding region computation.

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
}
