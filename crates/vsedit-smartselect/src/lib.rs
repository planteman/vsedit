//! Expand/shrink selection.

use std::fmt;

/// Errors that can occur when constructing or manipulating selection ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionError {
    /// The end position is before the start position.
    InvalidRange {
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    },
    /// An empty list of ranges was provided where at least one is required.
    EmptyRanges,
    /// A child range is not contained within its parent.
    ChildExceedsParent,
}

impl fmt::Display for SelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange { start_line, start_col, end_line, end_col } => {
                write!(
                    f,
                    "invalid range: start {}:{} is after end {}:{}",
                    start_line, start_col, end_line, end_col
                )
            }
            Self::EmptyRanges => write!(f, "ranges must not be empty"),
            Self::ChildExceedsParent => {
                write!(f, "child range is not contained within its parent")
            }
        }
    }
}

impl std::error::Error for SelectionError {}

/// A hierarchical selection range with an optional parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub parent: Option<Box<SelectionRange>>,
}

impl SelectionRange {
    /// Create a new range with no parent.
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self { start_line, start_col, end_line, end_col, parent: None }
    }

    /// Builder method to attach a parent range.
    pub fn with_parent(mut self, parent: SelectionRange) -> Self {
        self.parent = Some(Box::new(parent));
        self
    }

    /// Returns `true` when start equals end.
    pub fn is_empty(&self) -> bool {
        self.start_line == self.end_line && self.start_col == self.end_col
    }

    /// Number of lines spanned by this range.
    pub fn line_count(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Returns `true` when the range is on a single line.
    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }

    /// Counts the depth of the parent chain (0 if no parent).
    pub fn depth(&self) -> usize {
        let mut d = 0;
        let mut cur = self;
        while let Some(ref p) = cur.parent {
            d += 1;
            cur = p;
        }
        d
    }

    /// Walks to the outermost parent, returning a reference to it.
    pub fn outermost(&self) -> &SelectionRange {
        let mut cur = self;
        while let Some(ref p) = cur.parent {
            cur = p;
        }
        cur
    }
}

impl fmt::Display for SelectionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}:{} - {}:{}]",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
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

/// Check whether two ranges overlap (share at least one position).
pub fn selection_intersects(a: &SelectionRange, b: &SelectionRange) -> bool {
    let a_start = (a.start_line, a.start_col);
    let a_end = (a.end_line, a.end_col);
    let b_start = (b.start_line, b.start_col);
    let b_end = (b.end_line, b.end_col);
    a_start < b_end && b_start < a_end
}

/// Build a parent chain from a vec of `(start_line, start_col, end_line, end_col)`.
///
/// The first element becomes the innermost range; each subsequent element
/// becomes the parent of the previous one (i.e. outermost is last).
pub fn build_selection_chain(ranges: Vec<(u32, u32, u32, u32)>) -> SelectionRange {
    let mut iter = ranges.into_iter().rev();
    let (sl, sc, el, ec) = iter.next().expect("ranges must not be empty");
    let mut current = SelectionRange::new(sl, sc, el, ec);
    for (sl, sc, el, ec) in iter {
        current = SelectionRange::new(sl, sc, el, ec).with_parent(current);
    }
    current
}

/// Validated version of [`build_selection_chain`] that returns an error on
/// invalid input instead of panicking.
pub fn try_build_selection_chain(
    ranges: Vec<(u32, u32, u32, u32)>,
) -> Result<SelectionRange, SelectionError> {
    if ranges.is_empty() {
        return Err(SelectionError::EmptyRanges);
    }
    for &(sl, sc, el, ec) in &ranges {
        if (sl, sc) > (el, ec) {
            return Err(SelectionError::InvalidRange {
                start_line: sl,
                start_col: sc,
                end_line: el,
                end_col: ec,
            });
        }
    }
    Ok(build_selection_chain(ranges))
}

/// Compute the smallest range that contains both `a` and `b`.
pub fn selection_union(a: &SelectionRange, b: &SelectionRange) -> SelectionRange {
    let start = std::cmp::min((a.start_line, a.start_col), (b.start_line, b.start_col));
    let end = std::cmp::max((a.end_line, a.end_col), (b.end_line, b.end_col));
    SelectionRange::new(start.0, start.1, end.0, end.1)
}

/// Compute the intersection of two ranges, or `None` if they don't overlap.
pub fn selection_intersection(
    a: &SelectionRange,
    b: &SelectionRange,
) -> Option<SelectionRange> {
    if !selection_intersects(a, b) {
        return None;
    }
    let start = std::cmp::max((a.start_line, a.start_col), (b.start_line, b.start_col));
    let end = std::cmp::min((a.end_line, a.end_col), (b.end_line, b.end_col));
    Some(SelectionRange::new(start.0, start.1, end.0, end.1))
}

/// Collect all ranges in the parent chain into a `Vec`, innermost first.
pub fn collect_chain(range: &SelectionRange) -> Vec<&SelectionRange> {
    let mut out = Vec::new();
    let mut cur = range;
    out.push(cur);
    while let Some(ref p) = cur.parent {
        out.push(p);
        cur = p;
    }
    out
}

impl SelectionRange {
    /// Validated constructor that returns an error if start is after end.
    pub fn try_new(
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> Result<Self, SelectionError> {
        if (start_line, start_col) > (end_line, end_col) {
            return Err(SelectionError::InvalidRange {
                start_line,
                start_col,
                end_line,
                end_col,
            });
        }
        Ok(Self::new(start_line, start_col, end_line, end_col))
    }

    /// Attach a parent, validating that the parent fully contains this range.
    pub fn try_with_parent(
        self,
        parent: SelectionRange,
    ) -> Result<Self, SelectionError> {
        if !selection_contains(&parent, &self) {
            return Err(SelectionError::ChildExceedsParent);
        }
        Ok(self.with_parent(parent))
    }

    /// Returns `true` if this range fully contains `other`.
    pub fn contains(&self, other: &SelectionRange) -> bool {
        selection_contains(self, other)
    }

    /// Returns `true` if this range overlaps with `other`.
    pub fn intersects(&self, other: &SelectionRange) -> bool {
        selection_intersects(self, other)
    }

    /// Translate this range by a line delta (may be negative).
    pub fn translate_lines(&self, delta: i64) -> Option<SelectionRange> {
        let sl = (self.start_line as i64).checked_add(delta)?;
        let el = (self.end_line as i64).checked_add(delta)?;
        if sl < 0 || el < 0 {
            return None;
        }
        Some(SelectionRange {
            start_line: sl as u32,
            start_col: self.start_col,
            end_line: el as u32,
            end_col: self.end_col,
            parent: None,
        })
    }

    /// Returns the (line, col) of the start position as a tuple.
    pub fn start(&self) -> (u32, u32) {
        (self.start_line, self.start_col)
    }

    /// Returns the (line, col) of the end position as a tuple.
    pub fn end(&self) -> (u32, u32) {
        (self.end_line, self.end_col)
    }
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

    #[test]
    fn new_constructor() {
        let r = SelectionRange::new(1, 2, 3, 4);
        assert_eq!(r.start_line, 1);
        assert_eq!(r.start_col, 2);
        assert_eq!(r.end_line, 3);
        assert_eq!(r.end_col, 4);
        assert!(r.parent.is_none());
    }

    #[test]
    fn with_parent_builder() {
        let parent = SelectionRange::new(0, 0, 10, 0);
        let child = SelectionRange::new(2, 5, 4, 10).with_parent(parent.clone());
        assert_eq!(child.parent.as_deref(), Some(&parent));
    }

    #[test]
    fn is_empty_range() {
        assert!(SelectionRange::new(5, 3, 5, 3).is_empty());
        assert!(!SelectionRange::new(5, 3, 5, 4).is_empty());
    }

    #[test]
    fn line_count_and_single_line() {
        let single = SelectionRange::new(3, 0, 3, 20);
        assert_eq!(single.line_count(), 1);
        assert!(single.is_single_line());

        let multi = SelectionRange::new(3, 0, 8, 0);
        assert_eq!(multi.line_count(), 6);
        assert!(!multi.is_single_line());
    }

    #[test]
    fn depth_and_outermost() {
        let chain = sample_chain(); // depth: word -> line -> block
        assert_eq!(chain.depth(), 2);
        let outer = chain.outermost();
        assert_eq!(outer.start_line, 3);
        assert_eq!(outer.end_line, 8);

        let flat = SelectionRange::new(0, 0, 1, 0);
        assert_eq!(flat.depth(), 0);
        assert!(std::ptr::eq(flat.outermost(), &flat));
    }

    #[test]
    fn intersects() {
        let a = SelectionRange::new(1, 0, 5, 0);
        let b = SelectionRange::new(4, 0, 8, 0);
        assert!(selection_intersects(&a, &b));
        assert!(selection_intersects(&b, &a));

        let c = SelectionRange::new(5, 0, 8, 0);
        // a ends at (5,0) and c starts at (5,0) — not overlapping (half-open).
        assert!(!selection_intersects(&a, &c));

        let d = SelectionRange::new(10, 0, 12, 0);
        assert!(!selection_intersects(&a, &d));
    }

    #[test]
    fn build_chain() {
        let chain = build_selection_chain(vec![
            (5, 10, 5, 15),
            (5, 0, 5, 40),
            (3, 0, 8, 0),
        ]);
        assert_eq!(chain.start_col, 10);
        assert_eq!(chain.depth(), 2);
        let outer = chain.outermost();
        assert_eq!(outer.start_line, 3);
    }

    #[test]
    fn display_format() {
        let r = SelectionRange::new(1, 5, 3, 10);
        assert_eq!(format!("{r}"), "[1:5 - 3:10]");
    }

    #[test]
    fn try_new_valid() {
        let r = SelectionRange::try_new(1, 0, 5, 10).unwrap();
        assert_eq!(r.start(), (1, 0));
        assert_eq!(r.end(), (5, 10));
    }

    #[test]
    fn try_new_invalid() {
        let err = SelectionRange::try_new(5, 10, 3, 0).unwrap_err();
        assert_eq!(
            err,
            SelectionError::InvalidRange {
                start_line: 5,
                start_col: 10,
                end_line: 3,
                end_col: 0
            }
        );
        assert!(format!("{err}").contains("invalid range"));
    }

    #[test]
    fn try_with_parent_ok() {
        let parent = SelectionRange::new(0, 0, 10, 0);
        let child = SelectionRange::new(2, 5, 4, 10);
        let result = child.try_with_parent(parent).unwrap();
        assert_eq!(result.depth(), 1);
    }

    #[test]
    fn try_with_parent_err() {
        let parent = SelectionRange::new(3, 0, 4, 0);
        let child = SelectionRange::new(1, 0, 10, 0);
        let err = child.try_with_parent(parent).unwrap_err();
        assert_eq!(err, SelectionError::ChildExceedsParent);
    }

    #[test]
    fn try_build_chain_empty() {
        let err = try_build_selection_chain(vec![]).unwrap_err();
        assert_eq!(err, SelectionError::EmptyRanges);
    }

    #[test]
    fn try_build_chain_invalid_range() {
        let err = try_build_selection_chain(vec![(5, 0, 3, 0)]).unwrap_err();
        matches!(err, SelectionError::InvalidRange { .. });
    }

    #[test]
    fn try_build_chain_valid() {
        let chain = try_build_selection_chain(vec![
            (5, 10, 5, 15),
            (5, 0, 5, 40),
        ])
        .unwrap();
        assert_eq!(chain.depth(), 1);
        assert_eq!(chain.start_col, 10);
    }

    #[test]
    fn union_of_ranges() {
        let a = SelectionRange::new(3, 5, 6, 10);
        let b = SelectionRange::new(1, 0, 4, 20);
        let u = selection_union(&a, &b);
        assert_eq!(u.start(), (1, 0));
        assert_eq!(u.end(), (6, 10));
    }

    #[test]
    fn intersection_overlapping() {
        let a = SelectionRange::new(1, 0, 5, 10);
        let b = SelectionRange::new(3, 5, 8, 0);
        let i = selection_intersection(&a, &b).unwrap();
        assert_eq!(i.start(), (3, 5));
        assert_eq!(i.end(), (5, 10));
    }

    #[test]
    fn intersection_disjoint() {
        let a = SelectionRange::new(1, 0, 3, 0);
        let b = SelectionRange::new(5, 0, 8, 0);
        assert!(selection_intersection(&a, &b).is_none());
    }

    #[test]
    fn collect_chain_vec() {
        let chain = sample_chain();
        let collected = collect_chain(&chain);
        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0].start_col, 10); // innermost
        assert_eq!(collected[2].start_line, 3); // outermost
    }

    #[test]
    fn translate_lines_positive() {
        let r = SelectionRange::new(3, 5, 7, 10);
        let t = r.translate_lines(2).unwrap();
        assert_eq!(t.start(), (5, 5));
        assert_eq!(t.end(), (9, 10));
    }

    #[test]
    fn translate_lines_negative_underflow() {
        let r = SelectionRange::new(1, 0, 3, 0);
        assert!(r.translate_lines(-5).is_none());
    }

    #[test]
    fn contains_method_on_struct() {
        let outer = SelectionRange::new(0, 0, 10, 0);
        let inner = SelectionRange::new(2, 5, 4, 10);
        assert!(outer.contains(&inner));
        assert!(!inner.contains(&outer));
    }

    #[test]
    fn intersects_method_on_struct() {
        let a = SelectionRange::new(1, 0, 5, 0);
        let b = SelectionRange::new(4, 0, 8, 0);
        assert!(a.intersects(&b));
    }

    #[test]
    fn error_display() {
        let e = SelectionError::EmptyRanges;
        assert_eq!(format!("{e}"), "ranges must not be empty");

        let e2 = SelectionError::ChildExceedsParent;
        assert!(format!("{e2}").contains("child range"));
    }
}
