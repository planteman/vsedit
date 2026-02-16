//! High-level diff result types matching the VS Code diff editor model.

use similar::{ChangeTag, TextDiff};

/// A single character-level or word-level change within a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffChangeType {
    /// Unchanged text.
    Equal(String),
    /// Inserted text (present only in modified).
    Insert(String),
    /// Deleted text (present only in original).
    Delete(String),
}

/// A range of lines in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRange {
    /// 1-based start line.
    pub start: u32,
    /// Number of lines.
    pub count: u32,
}

/// A hunk of changes between two files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub old_range: LineRange,
    pub new_range: LineRange,
    pub changes: Vec<DiffChangeType>,
}

/// The result of computing a diff between two texts.
#[derive(Debug, Clone)]
pub struct DiffResult {
    pub hunks: Vec<DiffHunk>,
    pub additions: u32,
    pub deletions: u32,
    pub changes: u32,
}

/// A character-level change within a single line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineChange {
    pub tag: ChangeTag,
    pub text: String,
}

/// Compute a structured diff between original and modified text.
pub fn compute_diff(original: &str, modified: &str) -> DiffResult {
    let diff = TextDiff::from_lines(original, modified);
    let ops = diff.ops().to_vec();
    let mut hunks = Vec::new();
    let mut additions: u32 = 0;
    let mut deletions: u32 = 0;
    let mut change_count: u32 = 0;

    for op in &ops {
        match op {
            similar::DiffOp::Equal { .. } => {}
            similar::DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => {
                let mut changes = Vec::new();
                for i in *old_index..(*old_index + *old_len) {
                    if let Some(val) = diff.old_slices().get(i) {
                        changes.push(DiffChangeType::Delete(val.to_string()));
                    }
                }
                deletions += *old_len as u32;
                hunks.push(DiffHunk {
                    old_range: LineRange {
                        start: *old_index as u32 + 1,
                        count: *old_len as u32,
                    },
                    new_range: LineRange {
                        start: *new_index as u32 + 1,
                        count: 0,
                    },
                    changes,
                });
            }
            similar::DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => {
                let mut changes = Vec::new();
                for i in *new_index..(*new_index + *new_len) {
                    if let Some(val) = diff.new_slices().get(i) {
                        changes.push(DiffChangeType::Insert(val.to_string()));
                    }
                }
                additions += *new_len as u32;
                hunks.push(DiffHunk {
                    old_range: LineRange {
                        start: *old_index as u32 + 1,
                        count: 0,
                    },
                    new_range: LineRange {
                        start: *new_index as u32 + 1,
                        count: *new_len as u32,
                    },
                    changes,
                });
            }
            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let mut changes = Vec::new();
                for i in *old_index..(*old_index + *old_len) {
                    if let Some(val) = diff.old_slices().get(i) {
                        changes.push(DiffChangeType::Delete(val.to_string()));
                    }
                }
                for i in *new_index..(*new_index + *new_len) {
                    if let Some(val) = diff.new_slices().get(i) {
                        changes.push(DiffChangeType::Insert(val.to_string()));
                    }
                }
                deletions += *old_len as u32;
                additions += *new_len as u32;
                change_count += 1;
                hunks.push(DiffHunk {
                    old_range: LineRange {
                        start: *old_index as u32 + 1,
                        count: *old_len as u32,
                    },
                    new_range: LineRange {
                        start: *new_index as u32 + 1,
                        count: *new_len as u32,
                    },
                    changes,
                });
            }
        }
    }

    DiffResult {
        hunks,
        additions,
        deletions,
        changes: change_count,
    }
}

/// Compute character-level inline diff between two single lines.
pub fn compute_inline_diff(old_line: &str, new_line: &str) -> Vec<InlineChange> {
    let diff = TextDiff::from_chars(old_line, new_line);
    diff.iter_all_changes()
        .map(|c| InlineChange {
            tag: c.tag(),
            text: c.value().to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_diff_no_changes() {
        let result = compute_diff("hello\nworld\n", "hello\nworld\n");
        assert!(result.hunks.is_empty());
        assert_eq!(result.additions, 0);
        assert_eq!(result.deletions, 0);
    }

    #[test]
    fn compute_diff_insertion() {
        let result = compute_diff("a\nc\n", "a\nb\nc\n");
        assert_eq!(result.hunks.len(), 1);
        assert_eq!(result.additions, 1);
        assert_eq!(result.deletions, 0);
        assert!(matches!(&result.hunks[0].changes[0], DiffChangeType::Insert(_)));
    }

    #[test]
    fn compute_diff_deletion() {
        let result = compute_diff("a\nb\nc\n", "a\nc\n");
        assert_eq!(result.hunks.len(), 1);
        assert_eq!(result.deletions, 1);
        assert!(matches!(&result.hunks[0].changes[0], DiffChangeType::Delete(_)));
    }

    #[test]
    fn compute_diff_replacement() {
        let result = compute_diff("a\nb\n", "a\nB\n");
        assert_eq!(result.hunks.len(), 1);
        assert_eq!(result.changes, 1);
        // Should contain both a Delete and an Insert
        let has_delete = result.hunks[0].changes.iter().any(|c| matches!(c, DiffChangeType::Delete(_)));
        let has_insert = result.hunks[0].changes.iter().any(|c| matches!(c, DiffChangeType::Insert(_)));
        assert!(has_delete);
        assert!(has_insert);
    }

    #[test]
    fn compute_diff_line_ranges() {
        let result = compute_diff("a\nb\nc\n", "a\nx\ny\nc\n");
        assert_eq!(result.hunks.len(), 1);
        let hunk = &result.hunks[0];
        assert_eq!(hunk.old_range.start, 2);
        assert_eq!(hunk.old_range.count, 1);
        assert_eq!(hunk.new_range.start, 2);
        assert_eq!(hunk.new_range.count, 2);
    }

    #[test]
    fn compute_diff_empty_to_content() {
        let result = compute_diff("", "new\n");
        assert!(!result.hunks.is_empty());
        assert!(result.additions > 0);
    }

    #[test]
    fn compute_diff_content_to_empty() {
        let result = compute_diff("old\n", "");
        assert!(!result.hunks.is_empty());
        assert!(result.deletions > 0);
    }

    #[test]
    fn compute_diff_multiple_hunks() {
        let result = compute_diff("a\nb\nc\nd\ne\n", "a\nX\nc\nY\ne\n");
        assert_eq!(result.hunks.len(), 2);
    }

    #[test]
    fn inline_diff_basic() {
        let changes = compute_inline_diff("hello", "hallo");
        assert!(!changes.is_empty());
        let has_delete = changes.iter().any(|c| c.tag == ChangeTag::Delete);
        let has_insert = changes.iter().any(|c| c.tag == ChangeTag::Insert);
        assert!(has_delete);
        assert!(has_insert);
    }

    #[test]
    fn inline_diff_identical() {
        let changes = compute_inline_diff("same", "same");
        assert!(changes.iter().all(|c| c.tag == ChangeTag::Equal));
    }

    #[test]
    fn inline_diff_empty_to_text() {
        let changes = compute_inline_diff("", "added");
        assert!(changes.iter().all(|c| c.tag == ChangeTag::Insert));
    }
}
