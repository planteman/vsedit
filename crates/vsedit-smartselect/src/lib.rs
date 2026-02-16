//! Expand/shrink selection.

/// A hierarchical selection range with an optional parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub parent: Option<Box<SelectionRange>>,
}

/// Trait for language-aware selection range providers.
pub trait SelectionRangeProvider {
    fn provide_selection_ranges(
        &self,
        uri: &str,
        positions: &[(u32, u32)],
    ) -> Vec<SelectionRange>;
}

/// Expand the selection by returning its parent range.
pub fn expand_selection(current: &SelectionRange) -> Option<&SelectionRange> {
    current.parent.as_deref()
}

/// Shrink the selection: given the full chain starting at `root`, find the
/// deepest child whose parent equals `current`.
pub fn shrink_selection<'a>(
    root: &'a SelectionRange,
    current: &SelectionRange,
) -> Option<&'a SelectionRange> {
    // Walk down from root; keep track of the previous node.
    let mut prev = root;
    let mut node = root;
    loop {
        if node == current {
            return if std::ptr::eq(node, root) { None } else { Some(prev) };
        }
        match &node.parent {
            Some(p) => {
                prev = node;
                node = p;
            }
            None => return None,
        }
    }
}

/// Check whether `outer` fully contains `inner`.
pub fn selection_contains(outer: &SelectionRange, inner: &SelectionRange) -> bool {
    let outer_start = (outer.start_line, outer.start_col);
    let outer_end = (outer.end_line, outer.end_col);
    let inner_start = (inner.start_line, inner.start_col);
    let inner_end = (inner.end_line, inner.end_col);
    outer_start <= inner_start && inner_end <= outer_end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_chain() -> SelectionRange {
        // word -> line -> block (innermost has parent chain going outward)
        SelectionRange {
            start_line: 5,
            start_col: 10,
            end_line: 5,
            end_col: 15,
            parent: Some(Box::new(SelectionRange {
                start_line: 5,
                start_col: 0,
                end_line: 5,
                end_col: 40,
                parent: Some(Box::new(SelectionRange {
                    start_line: 3,
                    start_col: 0,
                    end_line: 8,
                    end_col: 0,
                    parent: None,
                })),
            })),
        }
    }

    #[test]
    fn expand() {
        let chain = sample_chain();
        let parent = expand_selection(&chain).unwrap();
        assert_eq!(parent.start_col, 0);
        assert_eq!(parent.end_col, 40);
    }

    #[test]
    fn shrink() {
        let chain = sample_chain();
        let line = expand_selection(&chain).unwrap(); // line-level
        let shrunk = shrink_selection(&chain, line).unwrap();
        assert_eq!(shrunk.start_col, 10); // back to word
    }

    #[test]
    fn contains() {
        let outer = SelectionRange {
            start_line: 1,
            start_col: 0,
            end_line: 10,
            end_col: 0,
            parent: None,
        };
        let inner = SelectionRange {
            start_line: 3,
            start_col: 5,
            end_line: 7,
            end_col: 10,
            parent: None,
        };
        assert!(selection_contains(&outer, &inner));
        assert!(!selection_contains(&inner, &outer));
    }
}
