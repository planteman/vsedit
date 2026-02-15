//! Diff computation algorithms.
//!
//! Equivalent to VS Code's diff computation.
//! Uses the `similar` crate for diffing text.

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
}
