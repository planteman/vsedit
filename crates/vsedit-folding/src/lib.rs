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
}

impl Default for FoldingModel {
    fn default() -> Self { Self::new() }
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
}
