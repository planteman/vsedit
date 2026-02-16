//! Dirty diff indicators for showing changes in the editor gutter.

use crate::diff_result::compute_diff;

/// Type of dirty diff decoration in the gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyDiffType {
    /// Line was added (green bar).
    Added,
    /// Line was modified (blue bar).
    Modified,
    /// Line was deleted (red arrow).
    Deleted,
}

/// A decoration to show in the editor gutter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyDiffDecoration {
    /// 1-based line number in the current file.
    pub line: u32,
    /// The type of change.
    pub decoration_type: DirtyDiffType,
}

/// Compute dirty diff decorations between original and current text.
pub fn compute_dirty_diff(original: &str, current: &str) -> Vec<DirtyDiffDecoration> {
    let diff = compute_diff(original, current);
    let mut decorations = Vec::new();

    for hunk in &diff.hunks {
        let old_count = hunk.old_range.count;
        let new_count = hunk.new_range.count;
        let new_start = hunk.new_range.start;

        if old_count == 0 && new_count > 0 {
            // Pure insertion
            for i in 0..new_count {
                decorations.push(DirtyDiffDecoration {
                    line: new_start + i,
                    decoration_type: DirtyDiffType::Added,
                });
            }
        } else if old_count > 0 && new_count == 0 {
            // Pure deletion — mark at the line where deletion happened
            decorations.push(DirtyDiffDecoration {
                line: new_start,
                decoration_type: DirtyDiffType::Deleted,
            });
        } else {
            // Modification — old lines replaced with new lines
            for i in 0..new_count {
                decorations.push(DirtyDiffDecoration {
                    line: new_start + i,
                    decoration_type: DirtyDiffType::Modified,
                });
            }
        }
    }

    decorations
}

/// Get the gutter indicator character for a dirty diff type.
pub fn gutter_indicator(decoration_type: DirtyDiffType) -> &'static str {
    match decoration_type {
        DirtyDiffType::Added => "┃",   // green bar
        DirtyDiffType::Modified => "┃", // blue bar
        DirtyDiffType::Deleted => "▼",  // red arrow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_diff_no_changes() {
        let decorations = compute_dirty_diff("hello\nworld\n", "hello\nworld\n");
        assert!(decorations.is_empty());
    }

    #[test]
    fn dirty_diff_added_lines() {
        let decorations = compute_dirty_diff("a\nc\n", "a\nb\nc\n");
        assert!(!decorations.is_empty());
        assert!(decorations.iter().all(|d| d.decoration_type == DirtyDiffType::Added));
    }

    #[test]
    fn dirty_diff_deleted_lines() {
        let decorations = compute_dirty_diff("a\nb\nc\n", "a\nc\n");
        assert!(!decorations.is_empty());
        assert!(decorations.iter().any(|d| d.decoration_type == DirtyDiffType::Deleted));
    }

    #[test]
    fn dirty_diff_modified_lines() {
        let decorations = compute_dirty_diff("a\nb\nc\n", "a\nX\nc\n");
        assert!(!decorations.is_empty());
        assert!(decorations.iter().any(|d| d.decoration_type == DirtyDiffType::Modified));
    }

    #[test]
    fn dirty_diff_multiple_changes() {
        let decorations = compute_dirty_diff("a\nb\nc\nd\n", "a\nX\nc\nY\nZ\n");
        assert!(decorations.len() >= 2);
    }

    #[test]
    fn gutter_indicators() {
        assert_eq!(gutter_indicator(DirtyDiffType::Added), "┃");
        assert_eq!(gutter_indicator(DirtyDiffType::Modified), "┃");
        assert_eq!(gutter_indicator(DirtyDiffType::Deleted), "▼");
    }
}
