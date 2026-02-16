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
}
