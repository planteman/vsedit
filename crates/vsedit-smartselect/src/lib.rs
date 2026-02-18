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

/// Statistics gathered from a history of selection expand/contract operations.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionStats {
    /// Total number of expansion operations recorded.
    pub total_expansions: u32,
    /// Total number of contraction operations recorded.
    pub total_contractions: u32,
    /// Average character-length of all selections in the history.
    pub avg_selection_length: f64,
    /// Maximum character-length observed across all selections.
    pub max_selection_length: u64,
}

/// A single entry in a selection history log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionHistoryEntry {
    /// The selection range at this point in time.
    pub range: SelectionRange,
    /// Whether this entry was produced by an expansion (`true`) or contraction (`false`).
    pub expanded: bool,
}

/// Compute [`SelectionStats`] from a slice of history entries.
///
/// Character-length is approximated as `(end_line - start_line) * line_width + (end_col - start_col)`
/// using the provided `line_width` estimate (e.g. 80 for a typical terminal).
///
/// Returns `None` if the history is empty.
pub fn compute_selection_stats(
    history: &[SelectionHistoryEntry],
    line_width: u64,
) -> Option<SelectionStats> {
    if history.is_empty() {
        return None;
    }

    let mut total_expansions: u32 = 0;
    let mut total_contractions: u32 = 0;
    let mut sum_length: u64 = 0;
    let mut max_length: u64 = 0;

    for entry in history {
        if entry.expanded {
            total_expansions += 1;
        } else {
            total_contractions += 1;
        }

        let r = &entry.range;
        let length = (r.end_line as u64).saturating_sub(r.start_line as u64) * line_width
            + (r.end_col as u64).saturating_sub(r.start_col as u64);

        sum_length += length;
        if length > max_length {
            max_length = length;
        }
    }

    Some(SelectionStats {
        total_expansions,
        total_contractions,
        avg_selection_length: sum_length as f64 / history.len() as f64,
        max_selection_length: max_length,
    })
}

/// A fixed anchor point within a document that a selection can be tethered to.
///
/// Anchors are useful for remembering a logical position (e.g. cursor origin)
/// while the selection range expands or contracts around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionAnchor {
    /// Line of the anchor position.
    pub line: u32,
    /// Column of the anchor position.
    pub col: u32,
}

impl SelectionAnchor {
    /// Create a new anchor at the given position.
    pub fn new(line: u32, col: u32) -> Self {
        Self { line, col }
    }

    /// Returns `true` if this anchor falls inside `range` (inclusive of boundaries).
    pub fn is_inside(&self, range: &SelectionRange) -> bool {
        let pos = (self.line, self.col);
        (range.start_line, range.start_col) <= pos && pos <= (range.end_line, range.end_col)
    }

    /// Find the deepest (innermost) range in the parent chain that still
    /// contains this anchor. Returns `None` if no range in the chain
    /// contains the anchor.
    pub fn deepest_containing<'a>(&self, range: &'a SelectionRange) -> Option<&'a SelectionRange> {
        // The chain is innermost-first: `range` is the deepest, parents go outward.
        // Return the first (deepest) range that contains the anchor.
        let mut cur = range;
        if self.is_inside(cur) {
            return Some(cur);
        }
        while let Some(ref p) = cur.parent {
            if self.is_inside(p) {
                return Some(p);
            }
            cur = p;
        }
        None
    }
}

impl fmt::Display for SelectionAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "anchor({}:{})", self.line, self.col)
    }
}

/// A stack that records selection history for undo/redo of expand/shrink.
#[derive(Debug, Clone)]
pub struct SelectionHistoryStack {
    entries: Vec<SelectionRange>,
    cursor: usize,
}

impl SelectionHistoryStack {
    /// Create an empty history stack.
    pub fn new() -> Self {
        Self { entries: Vec::new(), cursor: 0 }
    }

    /// Push a new selection onto the stack, discarding any forward history.
    pub fn push(&mut self, range: SelectionRange) {
        self.entries.truncate(self.cursor);
        self.entries.push(range);
        self.cursor = self.entries.len();
    }

    /// Move back in history, returning the previous selection.
    pub fn undo(&mut self) -> Option<&SelectionRange> {
        if self.cursor > 1 {
            self.cursor -= 1;
            Some(&self.entries[self.cursor - 1])
        } else {
            None
        }
    }

    /// Move forward in history, returning the next selection.
    pub fn redo(&mut self) -> Option<&SelectionRange> {
        if self.cursor < self.entries.len() {
            let r = &self.entries[self.cursor];
            self.cursor += 1;
            Some(r)
        } else {
            None
        }
    }

    /// Number of entries in the stack.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Current cursor position (1-based, 0 means nothing viewed yet).
    pub fn position(&self) -> usize {
        self.cursor
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = 0;
    }
}

/// Expand selections for multiple cursor positions simultaneously.
pub fn expand_multi_cursor(selections: &[SelectionRange]) -> Vec<Option<&SelectionRange>> {
    selections.iter().map(|s| expand_selection(s)).collect()
}

/// Detect the scope kind for a selection based on simple heuristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionScope {
    /// Selection is empty (cursor).
    Cursor,
    /// Selection spans part of a single line (likely a word or expression).
    SubLine,
    /// Selection covers exactly one full line.
    WholeLine,
    /// Selection spans multiple lines.
    MultiLine,
}

/// Classify a selection range into a [`SelectionScope`].
pub fn detect_scope(range: &SelectionRange) -> SelectionScope {
    if range.is_empty() {
        SelectionScope::Cursor
    } else if range.is_single_line() {
        SelectionScope::SubLine
    } else if range.start_col == 0 && range.end_col == 0 && range.line_count() == 2 {
        SelectionScope::WholeLine
    } else {
        SelectionScope::MultiLine
    }
}

/// Snap a selection's boundaries to the nearest line boundaries.
pub fn snap_to_line_boundaries(range: &SelectionRange) -> SelectionRange {
    SelectionRange::new(range.start_line, 0, range.end_line + 1, 0)
}

/// A stack for progressive expand/shrink selection through predefined levels.
#[derive(Debug, Clone)]
pub struct SelectionExpansionStack {
    levels: Vec<SelectionRange>,
    current_level: usize,
}

impl SelectionExpansionStack {
    /// Create an empty expansion stack.
    pub fn new() -> Self {
        Self { levels: Vec::new(), current_level: 0 }
    }

    /// Push a wider selection level onto the stack.
    pub fn push_level(&mut self, range: SelectionRange) {
        self.levels.push(range);
    }

    /// Move to the next wider level and return it.
    pub fn expand(&mut self) -> Option<&SelectionRange> {
        if self.current_level + 1 < self.levels.len() {
            self.current_level += 1;
            Some(&self.levels[self.current_level])
        } else {
            None
        }
    }

    /// Move to the next narrower level and return it.
    pub fn shrink(&mut self) -> Option<&SelectionRange> {
        if self.current_level > 0 {
            self.current_level -= 1;
            Some(&self.levels[self.current_level])
        } else {
            None
        }
    }

    /// Return the current level, if any.
    pub fn current(&self) -> Option<&SelectionRange> {
        self.levels.get(self.current_level)
    }

    /// Whether there is a wider level available.
    pub fn can_expand(&self) -> bool {
        self.current_level + 1 < self.levels.len()
    }

    /// Whether there is a narrower level available.
    pub fn can_shrink(&self) -> bool {
        self.current_level > 0
    }

    /// Total number of levels in the stack.
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Reset the cursor to the narrowest (first) level.
    pub fn reset(&mut self) {
        self.current_level = 0;
    }
}

impl Default for SelectionExpansionStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for smart-select providers that produce selection ranges from text.
pub trait SmartSelectProvider {
    /// Return a chain of selection ranges from narrowest to widest for the
    /// given position.
    fn provide_ranges(&self, text: &str, line: u32, col: u32) -> Vec<SelectionRange>;
}

/// Find the word boundaries around the given position and return a range.
///
/// A "word" is a contiguous run of alphanumeric or underscore characters.
pub fn find_word_at(text: &str, line: u32, col: u32) -> Option<SelectionRange> {
    let target_line = text.lines().nth(line as usize)?;
    let col = col as usize;
    if col > target_line.len() {
        return None;
    }
    let bytes = target_line.as_bytes();
    // Check that the cursor is on or adjacent to a word character.
    let at_word = col < bytes.len() && is_word_byte(bytes[col]);
    let before_word = col > 0 && is_word_byte(bytes[col - 1]);
    if !at_word && !before_word {
        return None;
    }
    let anchor = if at_word { col } else { col - 1 };
    let mut start = anchor;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = anchor;
    while end < bytes.len() && is_word_byte(bytes[end]) {
        end += 1;
    }
    if start == end {
        return None;
    }
    Some(SelectionRange::new(line, start as u32, line, end as u32))
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Return a range covering the entire line (including trailing newline position).
pub fn find_line_range(text: &str, line: u32) -> SelectionRange {
    let mut col_end = 0u32;
    if let Some(l) = text.lines().nth(line as usize) {
        col_end = l.len() as u32;
    }
    SelectionRange::new(line, 0, line, col_end)
}

/// Return a range covering the entire document.
pub fn find_document_range(text: &str) -> SelectionRange {
    let line_count = text.lines().count();
    if line_count == 0 {
        return SelectionRange::new(0, 0, 0, 0);
    }
    let last_line = (line_count - 1) as u32;
    let last_len = text.lines().last().map_or(0, |l| l.len()) as u32;
    SelectionRange::new(0, 0, last_line, last_len)
}

/// Find an indentation-based block around `line`.
///
/// The block includes all contiguous lines whose indentation is ≥ the
/// indentation of the anchor line, expanding outward until a less-indented
/// (or empty) line is found.
fn find_block_range(text: &str, line: u32) -> Option<SelectionRange> {
    let lines: Vec<&str> = text.lines().collect();
    let idx = line as usize;
    if idx >= lines.len() {
        return None;
    }
    let anchor_indent = indent_level(lines[idx]);
    if anchor_indent == 0 {
        return None; // top-level, no meaningful block
    }
    let mut start = idx;
    while start > 0 && indent_level(lines[start - 1]) >= anchor_indent {
        start -= 1;
    }
    let mut end = idx;
    while end + 1 < lines.len() && indent_level(lines[end + 1]) >= anchor_indent {
        end += 1;
    }
    let end_col = lines[end].len() as u32;
    Some(SelectionRange::new(start as u32, 0, end as u32, end_col))
}

fn indent_level(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// A basic provider that builds word → line → block → document ranges.
pub struct BasicSmartSelectProvider;

impl SmartSelectProvider for BasicSmartSelectProvider {
    fn provide_ranges(&self, text: &str, line: u32, col: u32) -> Vec<SelectionRange> {
        let mut ranges: Vec<SelectionRange> = Vec::new();
        if let Some(word) = find_word_at(text, line, col) {
            ranges.push(word);
        }
        let line_range = find_line_range(text, line);
        // Only add if it differs from the last pushed range.
        if ranges.last().map_or(true, |r| r != &line_range) {
            ranges.push(line_range);
        }
        if let Some(block) = find_block_range(text, line) {
            if ranges.last().map_or(true, |r| r != &block) {
                ranges.push(block);
            }
        }
        let doc = find_document_range(text);
        if ranges.last().map_or(true, |r| r != &doc) {
            ranges.push(doc);
        }
        ranges
    }
}

/// Build a syntax-aware selection chain from word → line → block → document.
///
/// Returns the innermost `SelectionRange` whose parent chain progresses to
/// progressively wider ranges.
pub fn syntax_aware_selection(text: &str, line: u32, col: u32) -> SelectionRange {
    let provider = BasicSmartSelectProvider;
    let ranges = provider.provide_ranges(text, line, col);
    if ranges.is_empty() {
        return find_document_range(text);
    }
    let tuples: Vec<(u32, u32, u32, u32)> = ranges
        .into_iter()
        .map(|r| (r.start_line, r.start_col, r.end_line, r.end_col))
        .collect();
    build_selection_chain(tuples)
}

// ── SelectionHistory ──

/// A complete history tracker that supports navigating through past selections
/// with undo and redo semantics.
#[derive(Debug, Clone)]
pub struct SelectionHistory {
    entries: Vec<SelectionHistoryEntry>,
    cursor: usize,
    max_size: usize,
}

impl SelectionHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            max_size,
        }
    }

    /// Record a selection change.  If the cursor is not at the end,
    /// redo history beyond the cursor is discarded.
    pub fn record(&mut self, range: SelectionRange, expanded: bool) {
        self.entries.truncate(self.cursor);
        self.entries.push(SelectionHistoryEntry { range, expanded });
        if self.entries.len() > self.max_size {
            let remove = self.entries.len() - self.max_size;
            self.entries.drain(0..remove);
        }
        self.cursor = self.entries.len();
    }

    /// Navigate to the previous selection.
    pub fn undo(&mut self) -> Option<&SelectionHistoryEntry> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        self.entries.get(self.cursor)
    }

    /// Navigate to the next selection.
    pub fn redo(&mut self) -> Option<&SelectionHistoryEntry> {
        if self.cursor >= self.entries.len() {
            return None;
        }
        let entry = self.entries.get(self.cursor);
        self.cursor += 1;
        entry
    }

    /// Return the current selection (the one at the cursor position).
    pub fn current(&self) -> Option<&SelectionHistoryEntry> {
        if self.cursor == 0 {
            return None;
        }
        self.entries.get(self.cursor - 1)
    }

    /// Whether undo is available.
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        self.cursor < self.entries.len()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return all entries.
    pub fn entries(&self) -> &[SelectionHistoryEntry] {
        &self.entries
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = 0;
    }
}

// ── Selection expansion heuristics ──

/// Levels of selection expansion from innermost to outermost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExpansionLevel {
    Word,
    Line,
    Block,
    Function,
    File,
}

impl fmt::Display for ExpansionLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Word => write!(f, "Word"),
            Self::Line => write!(f, "Line"),
            Self::Block => write!(f, "Block"),
            Self::Function => write!(f, "Function"),
            Self::File => write!(f, "File"),
        }
    }
}

/// Expand a selection to the next level using heuristics.
///
/// Returns `(new_range, level)` or `None` if the selection already covers
/// the entire file.
pub fn expand_to_next_level(
    text: &str,
    current: &SelectionRange,
) -> Option<(SelectionRange, ExpansionLevel)> {
    let doc = find_document_range(text);

    // If current covers the whole document, no further expansion
    if current.start_line == doc.start_line
        && current.start_col == doc.start_col
        && current.end_line == doc.end_line
        && current.end_col == doc.end_col
    {
        return None;
    }

    // If single-line and small, expand to full line
    if current.is_single_line() {
        let line_range = find_line_range(text, current.start_line);
        if line_range != *current {
            return Some((line_range, ExpansionLevel::Line));
        }
    }

    // Try expanding to a block (paragraph)
    if let Some(block) = find_block_range(text, current.start_line) {
        if selection_contains(&block, current) && block != *current {
            return Some((block, ExpansionLevel::Block));
        }
    }

    // Fall back to entire document
    Some((doc, ExpansionLevel::File))
}

/// Determine the current expansion level of a selection.
pub fn detect_expansion_level(
    text: &str,
    range: &SelectionRange,
) -> ExpansionLevel {
    let doc = find_document_range(text);
    if range.start_line == doc.start_line
        && range.start_col == doc.start_col
        && range.end_line == doc.end_line
        && range.end_col == doc.end_col
    {
        return ExpansionLevel::File;
    }

    if let Some(block) = find_block_range(text, range.start_line) {
        if range.start_line == block.start_line
            && range.end_line == block.end_line
        {
            return ExpansionLevel::Block;
        }
    }

    let line_range = find_line_range(text, range.start_line);
    if range.start_line == range.end_line
        && range.start_col == line_range.start_col
        && range.end_col == line_range.end_col
    {
        return ExpansionLevel::Line;
    }

    ExpansionLevel::Word
}

// ── Selection comparison and diffing ──

/// The result of comparing two selections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionDiff {
    /// Lines added (in new but not old).
    pub lines_added: i64,
    /// Columns shifted for start position.
    pub start_col_delta: i64,
    /// Columns shifted for end position.
    pub end_col_delta: i64,
    /// Whether the selection grew.
    pub grew: bool,
    /// Whether the selection moved (different start position).
    pub moved: bool,
}

/// Compare two selections and describe the difference.
pub fn selection_diff(old: &SelectionRange, new: &SelectionRange) -> SelectionDiff {
    let lines_added = (new.end_line as i64 - new.start_line as i64)
        - (old.end_line as i64 - old.start_line as i64);
    let start_col_delta = new.start_col as i64 - old.start_col as i64;
    let end_col_delta = new.end_col as i64 - old.end_col as i64;

    let old_span = (old.end_line as i64 - old.start_line as i64) * 1000
        + (old.end_col as i64 - old.start_col as i64);
    let new_span = (new.end_line as i64 - new.start_line as i64) * 1000
        + (new.end_col as i64 - new.start_col as i64);
    let grew = new_span > old_span;

    let moved = old.start_line != new.start_line || old.start_col != new.start_col;

    SelectionDiff {
        lines_added,
        start_col_delta,
        end_col_delta,
        grew,
        moved,
    }
}

/// Check whether two selections are identical in position (ignoring parent
/// chains).
pub fn selections_equal_position(a: &SelectionRange, b: &SelectionRange) -> bool {
    a.start_line == b.start_line
        && a.start_col == b.start_col
        && a.end_line == b.end_line
        && a.end_col == b.end_col
}

// ---------------------------------------------------------------------------
// Selection range utilities
// ---------------------------------------------------------------------------

/// Calculate the total character area (sum of per-line column spans) of a selection.
pub fn selection_char_count(range: &SelectionRange) -> u64 {
    if range.is_single_line() {
        return (range.end_col.saturating_sub(range.start_col)) as u64;
    }
    // Approximate: first line partial + middle full lines unknown + last line partial
    // We only have column info, so estimate conservatively
    let first_line = 80u64.saturating_sub(range.start_col as u64);
    let last_line = range.end_col as u64;
    let middle = if range.line_count() > 2 {
        (range.line_count() as u64 - 2) * 80
    } else {
        0
    };
    first_line + middle + last_line
}

/// Check if a selection range is a strict subset of another (proper containment).
pub fn is_strict_subset(inner: &SelectionRange, outer: &SelectionRange) -> bool {
    selection_contains(outer, inner) && !selections_equal_position(inner, outer)
}

/// Find the deepest chain depth among a set of selection ranges.
pub fn max_chain_depth(ranges: &[SelectionRange]) -> usize {
    ranges.iter().map(|r| r.depth()).max().unwrap_or(0)
}

/// Flatten a selection chain into a vector of (start_line, start_col, end_line, end_col) tuples.
pub fn flatten_chain(range: &SelectionRange) -> Vec<(u32, u32, u32, u32)> {
    collect_chain(range)
        .iter()
        .map(|r| (r.start_line, r.start_col, r.end_line, r.end_col))
        .collect()
}

/// Create a selection range that covers multiple ranges (bounding box).
pub fn bounding_range(ranges: &[SelectionRange]) -> Option<SelectionRange> {
    if ranges.is_empty() {
        return None;
    }
    let min_line = ranges.iter().map(|r| r.start_line).min().unwrap();
    let min_col = ranges
        .iter()
        .filter(|r| r.start_line == min_line)
        .map(|r| r.start_col)
        .min()
        .unwrap();
    let max_line = ranges.iter().map(|r| r.end_line).max().unwrap();
    let max_col = ranges
        .iter()
        .filter(|r| r.end_line == max_line)
        .map(|r| r.end_col)
        .max()
        .unwrap();
    Some(SelectionRange::new(min_line, min_col, max_line, max_col))
}

/// Check if two selection ranges are adjacent (one ends where the other begins on the same line).
pub fn selections_adjacent(a: &SelectionRange, b: &SelectionRange) -> bool {
    (a.end_line == b.start_line && a.end_col == b.start_col)
        || (b.end_line == a.start_line && b.end_col == a.start_col)
}

// ---------------------------------------------------------------------------
// SelectionSorter – utilities for ordering multi-cursor selections
// ---------------------------------------------------------------------------

/// Sort selection ranges by their start position (line, then column).
pub fn sort_selections(ranges: &mut [SelectionRange]) {
    ranges.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.start_col.cmp(&b.start_col))
    });
}

/// Remove duplicate selections (by position, ignoring parent chains).
pub fn dedup_selections(ranges: &mut Vec<SelectionRange>) {
    ranges.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.start_col.cmp(&b.start_col))
            .then(a.end_line.cmp(&b.end_line))
            .then(a.end_col.cmp(&b.end_col))
    });
    ranges.dedup_by(|a, b| {
        a.start_line == b.start_line
            && a.start_col == b.start_col
            && a.end_line == b.end_line
            && a.end_col == b.end_col
    });
}

/// Merge overlapping or adjacent selections into the smallest set of
/// non-overlapping ranges. Parent chains are discarded.
pub fn merge_overlapping_selections(ranges: &[SelectionRange]) -> Vec<SelectionRange> {
    if ranges.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<SelectionRange> = ranges.to_vec();
    sort_selections(&mut sorted);
    // Strip parents for merging
    for r in &mut sorted {
        r.parent = None;
    }
    let mut merged: Vec<SelectionRange> = vec![sorted[0].clone()];
    for r in &sorted[1..] {
        let last = merged.last_mut().unwrap();
        let last_end = (last.end_line, last.end_col);
        let r_start = (r.start_line, r.start_col);
        if r_start <= last_end {
            // overlapping or adjacent – extend
            if (r.end_line, r.end_col) > last_end {
                last.end_line = r.end_line;
                last.end_col = r.end_col;
            }
        } else {
            merged.push(r.clone());
        }
    }
    merged
}

/// Return the total number of characters selected across lines, given line contents.
/// Supports multi-line ranges.
pub fn selection_text_char_count(range: &SelectionRange, lines: &[&str]) -> usize {
    if range.start_line == range.end_line {
        let idx = range.start_line.saturating_sub(1) as usize;
        if idx < lines.len() {
            let s = range.start_col.saturating_sub(1) as usize;
            let e = range.end_col.saturating_sub(1) as usize;
            return e.saturating_sub(s);
        }
        return 0;
    }
    let mut count = 0usize;
    for ln in range.start_line..=range.end_line {
        let idx = ln.saturating_sub(1) as usize;
        if idx >= lines.len() {
            continue;
        }
        let line = lines[idx];
        if ln == range.start_line {
            let s = range.start_col.saturating_sub(1) as usize;
            count += line.len().saturating_sub(s);
        } else if ln == range.end_line {
            let e = range.end_col.saturating_sub(1) as usize;
            count += e.min(line.len());
        } else {
            count += line.len();
        }
    }
    count
}

/// Extract the selected text from line contents.
pub fn extract_selected_text(range: &SelectionRange, lines: &[&str]) -> String {
    if range.start_line == range.end_line {
        let idx = range.start_line.saturating_sub(1) as usize;
        if idx >= lines.len() {
            return String::new();
        }
        let line = lines[idx];
        let s = (range.start_col.saturating_sub(1) as usize).min(line.len());
        let e = (range.end_col.saturating_sub(1) as usize).min(line.len());
        return line[s..e].to_string();
    }
    let mut result = String::new();
    for ln in range.start_line..=range.end_line {
        let idx = ln.saturating_sub(1) as usize;
        if idx >= lines.len() {
            continue;
        }
        let line = lines[idx];
        if ln == range.start_line {
            let s = (range.start_col.saturating_sub(1) as usize).min(line.len());
            result.push_str(&line[s..]);
        } else if ln == range.end_line {
            result.push('\n');
            let e = (range.end_col.saturating_sub(1) as usize).min(line.len());
            result.push_str(&line[..e]);
        } else {
            result.push('\n');
            result.push_str(line);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// SmartSelectExpander – wraps expansion chain with strategy lookup
// ---------------------------------------------------------------------------

/// A strategy entry pairing an expansion level with a range.
#[derive(Debug, Clone)]
pub struct SmartSelectStrategy {
    pub level: ExpansionLevel,
    pub range: SelectionRange,
}

/// Builds and queries a chain of expansion strategies.
pub struct SmartSelectExpander {
    strategies: Vec<SmartSelectStrategy>,
}

impl SmartSelectExpander {
    /// Create from a list of `(level, range)` pairs; sorted by level.
    pub fn new(mut entries: Vec<(ExpansionLevel, SelectionRange)>) -> Self {
        entries.sort_by_key(|(lvl, _)| *lvl);
        let strategies = entries
            .into_iter()
            .map(|(level, range)| SmartSelectStrategy { level, range })
            .collect();
        Self { strategies }
    }

    /// Get the range for a specific level.
    pub fn range_for(&self, level: ExpansionLevel) -> Option<&SelectionRange> {
        self.strategies.iter().find(|s| s.level == level).map(|s| &s.range)
    }

    /// Expand from `current` to the next broader level available in strategies.
    pub fn expand_from(&self, current: ExpansionLevel) -> Option<&SmartSelectStrategy> {
        self.strategies.iter().find(|s| s.level > current)
    }

    /// Shrink from `current` to the next narrower level available.
    pub fn shrink_from(&self, current: ExpansionLevel) -> Option<&SmartSelectStrategy> {
        self.strategies.iter().rev().find(|s| s.level < current)
    }

    pub fn len(&self) -> usize {
        self.strategies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strategies.is_empty()
    }
}

// ---------------------------------------------------------------------------
// SmartSelectHistory – tracks expansion / shrink steps
// ---------------------------------------------------------------------------

/// Records the history of expand/shrink operations for undo support.
#[derive(Debug, Clone)]
pub struct SmartSelectHistory {
    stack: Vec<(ExpansionLevel, SelectionRange)>,
}

impl SmartSelectHistory {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Record an expansion step.
    pub fn push(&mut self, level: ExpansionLevel, range: SelectionRange) {
        self.stack.push((level, range));
    }

    /// Undo the most recent expansion.
    pub fn pop(&mut self) -> Option<(ExpansionLevel, SelectionRange)> {
        self.stack.pop()
    }

    /// Current (top) level.
    pub fn current_level(&self) -> Option<ExpansionLevel> {
        self.stack.last().map(|(l, _)| *l)
    }

    /// Peek at the current entry.
    pub fn current(&self) -> Option<&(ExpansionLevel, SelectionRange)> {
        self.stack.last()
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

impl Default for SmartSelectHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SmartSelectHint – UI hint for the next expansion
// ---------------------------------------------------------------------------

/// A hint showing the user what the next expand/shrink will select.
#[derive(Debug, Clone, PartialEq)]
pub struct SmartSelectHint {
    pub current_level: ExpansionLevel,
    pub next_level: Option<ExpansionLevel>,
    pub label: String,
}

impl SmartSelectHint {
    /// Build a hint from the current level.
    pub fn from_level(level: ExpansionLevel) -> Self {
        let next = match level {
            ExpansionLevel::Word => Some(ExpansionLevel::Line),
            ExpansionLevel::Line => Some(ExpansionLevel::Block),
            ExpansionLevel::Block => Some(ExpansionLevel::Function),
            ExpansionLevel::Function => Some(ExpansionLevel::File),
            ExpansionLevel::File => None,
        };
        let label = match next {
            Some(n) => format!("Expand to {}", n),
            None => "Already at broadest selection".to_string(),
        };
        Self { current_level: level, next_level: next, label }
    }

    /// Whether further expansion is possible.
    pub fn can_expand(&self) -> bool {
        self.next_level.is_some()
    }
}

impl fmt::Display for SmartSelectHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

// ---------------------------------------------------------------------------
// Bracket-aware selection helpers
// ---------------------------------------------------------------------------

/// A bracket pair for selection matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BracketPair {
    pub open: char,
    pub close: char,
}

impl BracketPair {
    pub const PARENS: Self = Self { open: '(', close: ')' };
    pub const BRACKETS: Self = Self { open: '[', close: ']' };
    pub const BRACES: Self = Self { open: '{', close: '}' };
    pub const ANGLES: Self = Self { open: '<', close: '>' };

    /// All common bracket pairs.
    pub fn all() -> &'static [BracketPair] {
        &[Self::PARENS, Self::BRACKETS, Self::BRACES, Self::ANGLES]
    }
}

/// Find the matching bracket range in a single line from a given offset.
/// Returns `(open_offset, close_offset)` inclusive, or `None`.
pub fn find_bracket_range(text: &str, offset: usize, pair: BracketPair) -> Option<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let mut depth = 0i32;
    let mut open_pos = None;
    let start = offset.min(chars.len().saturating_sub(1));
    for i in (0..=start).rev() {
        if chars[i] == pair.close {
            depth += 1;
        } else if chars[i] == pair.open {
            if depth == 0 {
                open_pos = Some(i);
                break;
            }
            depth -= 1;
        }
    }
    let open_pos = open_pos?;
    depth = 0;
    for i in (open_pos + 1)..chars.len() {
        if chars[i] == pair.open {
            depth += 1;
        } else if chars[i] == pair.close {
            if depth == 0 {
                return Some((open_pos, i));
            }
            depth -= 1;
        }
    }
    None
}


// ---------------------------------------------------------------------------
// SmartSelectBracketBalancer
// ---------------------------------------------------------------------------

/// A bracket pair definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketDef {
    pub open: char,
    pub close: char,
}

impl BracketDef {
    pub fn new(open: char, close: char) -> Self {
        Self { open, close }
    }
}

/// Expands selection to the nearest balanced bracket pair.
#[derive(Debug)]
pub struct SmartSelectBracketBalancer {
    bracket_defs: Vec<BracketDef>,
}

impl SmartSelectBracketBalancer {
    /// Create with the standard bracket set: `()`, `[]`, `{}`, `<>`.
    pub fn new() -> Self {
        Self {
            bracket_defs: vec![
                BracketDef::new('(', ')'),
                BracketDef::new('[', ']'),
                BracketDef::new('{', '}'),
                BracketDef::new('<', '>'),
            ],
        }
    }

    /// Create with a custom set of brackets.
    pub fn with_brackets(defs: Vec<BracketDef>) -> Self {
        Self { bracket_defs: defs }
    }

    /// Check if a character is an opening bracket.
    pub fn is_open(&self, ch: char) -> bool {
        self.bracket_defs.iter().any(|b| b.open == ch)
    }

    /// Check if a character is a closing bracket.
    pub fn is_close(&self, ch: char) -> bool {
        self.bracket_defs.iter().any(|b| b.close == ch)
    }

    /// Find the matching bracket character.
    pub fn matching_bracket(&self, ch: char) -> Option<char> {
        for b in &self.bracket_defs {
            if b.open == ch { return Some(b.close); }
            if b.close == ch { return Some(b.open); }
        }
        None
    }

    /// Expand selection outward to the next balanced bracket pair containing
    /// the given position in a single-line string.
    ///
    /// Returns `Some((open_idx, close_idx))` or `None` if no enclosing brackets are found.
    pub fn expand_at(&self, text: &str, position: usize) -> Option<(usize, usize)> {
        let chars: Vec<char> = text.chars().collect();
        if position >= chars.len() {
            return None;
        }
        // Search outward from position
        for radius in 1..chars.len() {
            let left = position.checked_sub(radius);
            if let Some(l) = left {
                if self.is_open(chars[l]) {
                    let expected_close = self.matching_bracket(chars[l])?;
                    // Find matching close
                    let mut depth = 1i32;
                    for r in (l + 1)..chars.len() {
                        if chars[r] == chars[l] {
                            depth += 1;
                        } else if chars[r] == expected_close {
                            depth -= 1;
                            if depth == 0 && r >= position {
                                return Some((l, r));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Check if brackets in a string are balanced.
    pub fn is_balanced(&self, text: &str) -> bool {
        let mut stack: Vec<char> = Vec::new();
        for ch in text.chars() {
            if self.is_open(ch) {
                stack.push(ch);
            } else if self.is_close(ch) {
                match stack.pop() {
                    Some(open) => {
                        if self.matching_bracket(open) != Some(ch) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
        }
        stack.is_empty()
    }

    /// Find all bracket pair positions in a string, returned as `(open_idx, close_idx, bracket_def_index)`.
    pub fn find_all_pairs(&self, text: &str) -> Vec<(usize, usize, usize)> {
        let chars: Vec<char> = text.chars().collect();
        let mut stack: Vec<(usize, usize)> = Vec::new(); // (char_idx, bracket_def_index)
        let mut pairs = Vec::new();
        for (i, &ch) in chars.iter().enumerate() {
            if let Some(def_idx) = self.bracket_defs.iter().position(|b| b.open == ch) {
                stack.push((i, def_idx));
            } else if let Some(def_idx) = self.bracket_defs.iter().position(|b| b.close == ch) {
                if let Some((open_idx, open_def_idx)) = stack.pop() {
                    if open_def_idx == def_idx {
                        pairs.push((open_idx, i, def_idx));
                    }
                }
            }
        }
        pairs
    }
}

// ---------------------------------------------------------------------------
// SmartSelectWordExtender
// ---------------------------------------------------------------------------

/// Classification of characters for word boundary detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharClass {
    Word,
    Whitespace,
    Punctuation,
    Digit,
}

/// Extends selection to word boundaries with language awareness.
#[derive(Debug)]
pub struct SmartSelectWordExtender {
    word_separators: Vec<char>,
    include_underscores_in_words: bool,
}

impl SmartSelectWordExtender {
    pub fn new() -> Self {
        Self {
            word_separators: vec![' ', '\t', '\n', '.', ',', ';', ':', '!', '?', '"', '\''],
            include_underscores_in_words: true,
        }
    }

    /// Create with custom separator set.
    pub fn with_separators(seps: Vec<char>, include_underscores: bool) -> Self {
        Self {
            word_separators: seps,
            include_underscores_in_words: include_underscores,
        }
    }

    /// Classify a character.
    pub fn classify(&self, ch: char) -> CharClass {
        if ch.is_whitespace() {
            CharClass::Whitespace
        } else if ch.is_ascii_digit() {
            CharClass::Digit
        } else if ch.is_alphanumeric() || (self.include_underscores_in_words && ch == '_') {
            CharClass::Word
        } else {
            CharClass::Punctuation
        }
    }

    /// Extend selection to the word at a given byte offset.
    /// Returns `(start, end)` byte range of the word, or `None` if position is out of range.
    pub fn word_at(&self, text: &str, position: usize) -> Option<(usize, usize)> {
        let chars: Vec<char> = text.chars().collect();
        if position >= chars.len() {
            return None;
        }
        let target_class = self.classify(chars[position]);
        if target_class == CharClass::Whitespace {
            return None;
        }
        let mut start = position;
        while start > 0 && self.classify(chars[start - 1]) == target_class {
            start -= 1;
        }
        let mut end = position;
        while end < chars.len() - 1 && self.classify(chars[end + 1]) == target_class {
            end += 1;
        }
        Some((start, end + 1))
    }

    /// Split text into words (runs of same-class non-whitespace characters).
    pub fn split_words(&self, text: &str) -> Vec<String> {
        let mut words = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let cls = self.classify(chars[i]);
            if cls == CharClass::Whitespace {
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len() && self.classify(chars[i]) == cls {
                i += 1;
            }
            words.push(chars[start..i].iter().collect());
        }
        words
    }

    /// Extend a selection outward to include surrounding whitespace on one side.
    pub fn extend_with_trailing_space(&self, text: &str, start: usize, end: usize) -> (usize, usize) {
        let chars: Vec<char> = text.chars().collect();
        let mut new_end = end;
        while new_end < chars.len() && chars[new_end].is_whitespace() {
            new_end += 1;
        }
        if new_end == end {
            let mut new_start = start;
            while new_start > 0 && chars[new_start - 1].is_whitespace() {
                new_start -= 1;
            }
            return (new_start, end);
        }
        (start, new_end)
    }
}



// ---------------------------------------------------------------------------
// vsedit-smartselect: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartselectXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl SmartselectXConfig {
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

impl std::fmt::Display for SmartselectXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct SmartselectXRegistry {
    entries: Vec<SmartselectXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl SmartselectXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: SmartselectXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&SmartselectXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut SmartselectXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<SmartselectXConfig> {
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

    pub fn active_entries(&self) -> Vec<&SmartselectXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&SmartselectXConfig> {
        let mut sorted: Vec<&SmartselectXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&SmartselectXConfig> {
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

    pub fn iter(&self) -> SmartselectXIterator<'_> {
        SmartselectXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct SmartselectXIterator<'a> {
    inner: std::slice::Iter<'a, SmartselectXConfig>,
}

impl<'a> Iterator for SmartselectXIterator<'a> {
    type Item = &'a SmartselectXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct SmartselectXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl SmartselectXCache {
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
pub struct SmartselectXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl SmartselectXFormatter {
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

    pub fn format_entry(&self, entry: &SmartselectXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &SmartselectXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &SmartselectXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for SmartselectXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct SmartselectXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl SmartselectXValidator {
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

    pub fn validate(&self, entry: &SmartselectXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &SmartselectXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for SmartselectXValidator {
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
// xa_ extended helpers for smartselect
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaSmartselectRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaSmartselectRingBuf {
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
pub struct XaSmartselectCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaSmartselectCounter {
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

impl Default for XaSmartselectCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 160
// ---------------------------------------------------------------------------

/// Generic object pool `Xc160Pool<T>`.
pub struct Xc160Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc160Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc160PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc160Pool<T> {
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
    pub fn stats(&self) -> Xc160PoolStats {
        Xc160PoolStats {
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

impl<T> Default for Xc160Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc160Scheduler`.
pub struct Xc160Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc160Scheduler {
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

impl Default for Xc160Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_160 hash for the given byte slice.
pub fn xc_160_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_160 convention.
pub fn xc_160_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_84 deepening: state machine + event bus ---

/// States for the Xd84 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd84State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd84State {
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
pub struct Xd84Transition {
    pub from: Xd84State,
    pub to: Xd84State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd84StateMachine {
    current: Xd84State,
    history: Vec<Xd84Transition>,
    step_counter: usize,
}

impl Xd84StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd84State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd84State {
        self.current
    }

    pub fn history(&self) -> &[Xd84Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd84State) -> Result<Xd84State, String> {
        let allowed = match (self.current, target) {
            (Xd84State::Idle, Xd84State::Running) => true,
            (Xd84State::Running, Xd84State::Paused) => true,
            (Xd84State::Running, Xd84State::Done) => true,
            (Xd84State::Paused, Xd84State::Running) => true,
            (Xd84State::Paused, Xd84State::Done) => true,
            (Xd84State::Done, Xd84State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_84: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd84Transition {
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
            "Xd84SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd84State> {
        let prefix = "Xd84SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd84State::Idle),
            "Running" => Some(Xd84State::Running),
            "Paused" => Some(Xd84State::Paused),
            "Done" => Some(Xd84State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd84State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd84 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd84Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd84Event {
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

type Xd84HandlerFn = Box<dyn Fn(&Xd84Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd84EventBus {
    handlers: Vec<(usize, Option<String>, Xd84HandlerFn)>,
    next_id: usize,
    published: Vec<Xd84Event>,
}

impl Xd84EventBus {
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
        F: Fn(&Xd84Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd84Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd84Event) {
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

    pub fn published_events(&self) -> &[Xd84Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #105
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf105Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf105TrieNode {
    children: std::collections::HashMap<char, Xf105TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf105Trie {
    root: Xf105TrieNode,
    count: usize,
}

impl Xf105Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf105TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf105TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf105TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf105BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf105BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 159).
pub struct Xh159SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh159SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 201 as u64,
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

/// A compact bit set supporting boolean operations (variant 159).
pub struct Xh159BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh159BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 159).
pub struct Xi159Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi159Deque<T> {
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
pub struct Xi159Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi159Interval {
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

/// A simple interval tree (variant 159).
pub struct Xi159IntervalTree {
    xi_intervals: Vec<Xi159Interval>,
}

impl Xi159IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi159Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi159Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi159Interval) -> Vec<&Xi159Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi159Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi159Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi159Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi159Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi159Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi159Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 159) ---

/// Disjoint set / union-find for crate 159.
pub struct Xj159UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj159UnionFind {
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

const XJ159_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 159.
pub struct Xj159BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj159BTreeNode<K, V>>>,
    len: usize,
}

struct Xj159BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj159BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj159BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ159_BTREE_ORDER - 1
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
        let mid = XJ159_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj159BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj159BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj159BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj159BTreeNode::xj_new_leaf();
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


// --- xk_159 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk159SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk159SegmentTree {
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
pub struct Xk159DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk159DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_159).
#[derive(Debug, Clone)]
pub struct Xl159Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl159Rope {
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

/// Suffix array for efficient string searching (xl_159).
#[derive(Debug, Clone)]
pub struct Xl159SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl159SuffixArray {
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
pub struct Xm159MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm159MatrixSparse {
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
pub struct Xm159Tokenizer {
    text: String,
}

impl Xm159Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 159.
pub struct Xn159Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn159Fenwick {
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

// ----- AVL tree map — crate 159 -----

#[derive(Debug, Clone)]
struct Xn159AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn159AvlNode<K, V>>>,
    right: Option<Box<Xn159AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 159.
#[derive(Debug, Clone)]
pub struct Xn159AVL<K, V> {
    root: Option<Box<Xn159AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn159AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn159AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn159AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn159AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn159AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn159AvlNode<K, V>>) -> Box<Xn159AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn159AvlNode<K, V>>) -> Box<Xn159AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn159AvlNode<K, V>>) -> Box<Xn159AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn159AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn159AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn159AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn159AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn159AvlNode<K, V>>) -> &Xn159AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn159AvlNode<K, V>>) -> (Box<Xn159AvlNode<K, V>>, Option<Box<Xn159AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn159AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn159AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn159AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn159AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn159AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn159AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn159AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    // --- SelectionStats / compute_selection_stats tests ---

    #[test]
    fn stats_empty_history() {
        assert!(compute_selection_stats(&[], 80).is_none());
    }

    #[test]
    fn stats_single_expansion() {
        let history = vec![SelectionHistoryEntry {
            range: SelectionRange::new(1, 0, 1, 20),
            expanded: true,
        }];
        let stats = compute_selection_stats(&history, 80).unwrap();
        assert_eq!(stats.total_expansions, 1);
        assert_eq!(stats.total_contractions, 0);
        assert!((stats.avg_selection_length - 20.0).abs() < f64::EPSILON);
        assert_eq!(stats.max_selection_length, 20);
    }

    #[test]
    fn stats_mixed_history() {
        let history = vec![
            SelectionHistoryEntry {
                range: SelectionRange::new(1, 0, 1, 10),
                expanded: true,
            },
            SelectionHistoryEntry {
                range: SelectionRange::new(1, 0, 3, 10),
                expanded: true,
            },
            SelectionHistoryEntry {
                range: SelectionRange::new(1, 5, 1, 15),
                expanded: false,
            },
        ];
        let stats = compute_selection_stats(&history, 80).unwrap();
        assert_eq!(stats.total_expansions, 2);
        assert_eq!(stats.total_contractions, 1);
        // lengths: 10, 2*80+10=170, 10 → avg = 190/3 ≈ 63.33
        assert!((stats.avg_selection_length - 190.0 / 3.0).abs() < 0.01);
        assert_eq!(stats.max_selection_length, 170);
    }

    // --- SelectionAnchor tests ---

    #[test]
    fn anchor_inside_range() {
        let anchor = SelectionAnchor::new(5, 10);
        let range = SelectionRange::new(3, 0, 8, 0);
        assert!(anchor.is_inside(&range));

        let outside = SelectionAnchor::new(10, 0);
        assert!(!outside.is_inside(&range));
    }

    #[test]
    fn anchor_at_boundary() {
        let anchor_start = SelectionAnchor::new(3, 0);
        let anchor_end = SelectionAnchor::new(8, 0);
        let range = SelectionRange::new(3, 0, 8, 0);
        assert!(anchor_start.is_inside(&range));
        assert!(anchor_end.is_inside(&range));
    }

    #[test]
    fn anchor_deepest_containing() {
        let chain = sample_chain(); // word [5:10-5:15] -> line [5:0-5:40] -> block [3:0-8:0]
        let anchor = SelectionAnchor::new(5, 12);
        // The deepest (innermost) range containing the anchor is the word range itself.
        let deepest = anchor.deepest_containing(&chain).unwrap();
        assert_eq!(deepest.start_col, 10);
        assert_eq!(deepest.end_col, 15);

        // Anchor outside all ranges returns None.
        let far = SelectionAnchor::new(20, 0);
        assert!(far.deepest_containing(&chain).is_none());
    }

    #[test]
    fn anchor_display() {
        let a = SelectionAnchor::new(7, 3);
        assert_eq!(format!("{a}"), "anchor(7:3)");
    }

    #[test]
    fn history_stack_push_and_undo() {
        let mut stack = SelectionHistoryStack::new();
        assert!(stack.is_empty());
        stack.push(SelectionRange::new(1, 0, 1, 5));
        stack.push(SelectionRange::new(1, 0, 1, 20));
        assert_eq!(stack.len(), 2);
        let prev = stack.undo().unwrap();
        assert_eq!(prev.end_col, 5);
    }

    #[test]
    fn history_stack_redo() {
        let mut stack = SelectionHistoryStack::new();
        stack.push(SelectionRange::new(1, 0, 1, 5));
        stack.push(SelectionRange::new(1, 0, 1, 20));
        stack.undo();
        let next = stack.redo().unwrap();
        assert_eq!(next.end_col, 20);
        assert!(stack.redo().is_none());
    }

    #[test]
    fn history_stack_push_truncates_forward() {
        let mut stack = SelectionHistoryStack::new();
        stack.push(SelectionRange::new(1, 0, 1, 5));
        stack.push(SelectionRange::new(1, 0, 1, 20));
        stack.undo();
        stack.push(SelectionRange::new(2, 0, 2, 10));
        assert_eq!(stack.len(), 2);
        assert!(stack.redo().is_none());
    }

    #[test]
    fn history_stack_clear() {
        let mut stack = SelectionHistoryStack::new();
        stack.push(SelectionRange::new(1, 0, 1, 5));
        stack.clear();
        assert!(stack.is_empty());
        assert_eq!(stack.position(), 0);
    }

    #[test]
    fn expand_multi_cursor_works() {
        let chain1 = SelectionRange::new(1, 0, 1, 5)
            .with_parent(SelectionRange::new(1, 0, 1, 40));
        let chain2 = SelectionRange::new(5, 0, 5, 3);
        let chains = [chain1, chain2];
        let results = expand_multi_cursor(&chains);
        assert!(results[0].is_some());
        assert!(results[1].is_none());
    }

    #[test]
    fn detect_scope_cursor() {
        let r = SelectionRange::new(3, 5, 3, 5);
        assert_eq!(detect_scope(&r), SelectionScope::Cursor);
    }

    #[test]
    fn detect_scope_subline() {
        let r = SelectionRange::new(3, 5, 3, 15);
        assert_eq!(detect_scope(&r), SelectionScope::SubLine);
    }

    #[test]
    fn detect_scope_multiline() {
        let r = SelectionRange::new(1, 0, 5, 10);
        assert_eq!(detect_scope(&r), SelectionScope::MultiLine);
    }

    #[test]
    fn snap_to_line_boundaries_works() {
        let r = SelectionRange::new(3, 5, 7, 15);
        let snapped = snap_to_line_boundaries(&r);
        assert_eq!(snapped.start_col, 0);
        assert_eq!(snapped.end_line, 8);
        assert_eq!(snapped.end_col, 0);
    }

    #[test]
    fn stats_multiline_max_length() {
        let history = vec![
            SelectionHistoryEntry {
                range: SelectionRange::new(0, 0, 0, 5),
                expanded: true,
            },
            SelectionHistoryEntry {
                range: SelectionRange::new(0, 0, 10, 5),
                expanded: true,
            },
        ];
        let stats = compute_selection_stats(&history, 100).unwrap();
        // lengths: 5, 10*100+5=1005
        assert_eq!(stats.max_selection_length, 1005);
        assert_eq!(stats.total_expansions, 2);
        assert_eq!(stats.total_contractions, 0);
    }

    // ---- SelectionExpansionStack tests ----

    #[test]
    fn expansion_stack_expand_and_shrink() {
        let mut stack = SelectionExpansionStack::new();
        stack.push_level(SelectionRange::new(1, 0, 1, 5));
        stack.push_level(SelectionRange::new(1, 0, 1, 20));
        stack.push_level(SelectionRange::new(0, 0, 3, 0));

        assert_eq!(stack.level_count(), 3);
        assert_eq!(stack.current().unwrap(), &SelectionRange::new(1, 0, 1, 5));

        assert!(stack.can_expand());
        let r = stack.expand().unwrap();
        assert_eq!(r, &SelectionRange::new(1, 0, 1, 20));

        let r = stack.expand().unwrap();
        assert_eq!(r, &SelectionRange::new(0, 0, 3, 0));
        assert!(!stack.can_expand());
        assert!(stack.expand().is_none());

        assert!(stack.can_shrink());
        let r = stack.shrink().unwrap();
        assert_eq!(r, &SelectionRange::new(1, 0, 1, 20));

        stack.reset();
        assert_eq!(stack.current().unwrap(), &SelectionRange::new(1, 0, 1, 5));
        assert!(!stack.can_shrink());
    }

    #[test]
    fn expansion_stack_empty() {
        let stack = SelectionExpansionStack::new();
        assert_eq!(stack.level_count(), 0);
        assert!(stack.current().is_none());
        assert!(!stack.can_expand());
        assert!(!stack.can_shrink());
    }

    // ---- find_word_at tests ----

    #[test]
    fn find_word_at_simple() {
        let text = "hello world_foo bar";
        let r = find_word_at(text, 0, 7).unwrap();
        assert_eq!(r, SelectionRange::new(0, 6, 0, 15)); // "world_foo"
    }

    #[test]
    fn find_word_at_start_of_line() {
        let text = "abc def";
        let r = find_word_at(text, 0, 0).unwrap();
        assert_eq!(r, SelectionRange::new(0, 0, 0, 3));
    }

    #[test]
    fn find_word_at_no_word() {
        let text = "   ";
        assert!(find_word_at(text, 0, 1).is_none());
    }

    // ---- find_line_range / find_document_range tests ----

    #[test]
    fn find_line_range_basic() {
        let text = "first\nsecond\nthird";
        let r = find_line_range(text, 1);
        assert_eq!(r, SelectionRange::new(1, 0, 1, 6));
    }

    #[test]
    fn find_document_range_basic() {
        let text = "aaa\nbb\nc";
        let r = find_document_range(text);
        assert_eq!(r, SelectionRange::new(0, 0, 2, 1));
    }

    // ---- syntax_aware_selection tests ----

    #[test]
    fn syntax_aware_selection_builds_chain() {
        let text = "fn main() {\n    let x = 42;\n}\n";
        let sel = syntax_aware_selection(text, 1, 8);
        // innermost should be a word
        assert!(sel.is_single_line());
        // should have a parent chain
        assert!(sel.depth() >= 2, "expected depth >= 2, got {}", sel.depth());
        // outermost should be the document
        let outer = sel.outermost();
        assert_eq!(outer.start_line, 0);
        assert_eq!(outer.start_col, 0);
    }

    #[test]
    fn syntax_aware_selection_no_word() {
        // cursor on whitespace — should still produce line → doc chain
        let text = "hello\n    \nworld";
        let sel = syntax_aware_selection(text, 1, 2);
        assert!(sel.depth() >= 1);
    }

    // ---- BasicSmartSelectProvider tests ----

    #[test]
    fn basic_provider_ranges_ordering() {
        let provider = BasicSmartSelectProvider;
        let text = "fn foo() {\n    bar();\n}\n";
        let ranges = provider.provide_ranges(text, 1, 5);
        // Each successive range should be at least as wide as the previous.
        for window in ranges.windows(2) {
            let a = &window[0];
            let b = &window[1];
            assert!(
                selection_contains(b, a),
                "range {} should contain {}",
                b,
                a,
            );
        }
    }

    // ── SelectionHistory tests ──

    #[test]
    fn selection_history_undo_redo() {
        let mut hist = SelectionHistory::new(100);
        let r1 = SelectionRange::new(1, 1, 1, 5);
        let r2 = SelectionRange::new(1, 1, 1, 10);
        hist.record(r1.clone(), true);
        hist.record(r2.clone(), true);
        assert_eq!(hist.len(), 2);
        assert!(hist.can_undo());

        let undone = hist.undo().unwrap();
        assert_eq!(undone.range, r2);
        assert!(hist.can_redo());

        let redone = hist.redo().unwrap();
        assert_eq!(redone.range, r2);
    }

    #[test]
    fn selection_history_record_clears_redo() {
        let mut hist = SelectionHistory::new(100);
        hist.record(SelectionRange::new(1, 1, 1, 5), true);
        hist.record(SelectionRange::new(1, 1, 1, 10), true);
        hist.undo();
        hist.record(SelectionRange::new(2, 1, 2, 5), false);
        assert!(!hist.can_redo());
        assert_eq!(hist.len(), 2);
    }

    // ── Expansion heuristics tests ──

    #[test]
    fn expand_to_next_level_from_word() {
        let text = "fn main() {\n    hello world\n}";
        let word = SelectionRange::new(2, 5, 2, 10);
        let result = expand_to_next_level(text, &word);
        assert!(result.is_some());
        let (expanded, _level) = result.unwrap();
        // The expanded range should be different from the original
        assert!(expanded != word);
    }

    #[test]
    fn expand_to_next_level_already_file() {
        let text = "hello";
        let doc = find_document_range(text);
        let result = expand_to_next_level(text, &doc);
        assert!(result.is_none());
    }

    #[test]
    fn detect_expansion_level_word() {
        let text = "fn main() {\n    hello world\n}";
        let word = SelectionRange::new(2, 5, 2, 10);
        let level = detect_expansion_level(text, &word);
        assert_eq!(level, ExpansionLevel::Word);
    }

    // ── Selection diff tests ──

    #[test]
    fn selection_diff_grew() {
        let old = SelectionRange::new(1, 1, 1, 5);
        let new = SelectionRange::new(1, 1, 1, 10);
        let diff = selection_diff(&old, &new);
        assert!(diff.grew);
        assert!(!diff.moved);
        assert_eq!(diff.end_col_delta, 5);
    }

    #[test]
    fn selection_diff_moved() {
        let old = SelectionRange::new(1, 1, 1, 5);
        let new = SelectionRange::new(2, 1, 2, 5);
        let diff = selection_diff(&old, &new);
        assert!(diff.moved);
    }

    #[test]
    fn selections_equal_position_test() {
        let a = SelectionRange::new(1, 1, 2, 5);
        let b = SelectionRange::new(1, 1, 2, 5).with_parent(SelectionRange::new(1, 1, 3, 1));
        assert!(selections_equal_position(&a, &b));
    }

    #[test]
    fn selection_char_count_single_line() {
        let r = SelectionRange::new(1, 5, 1, 15);
        assert_eq!(selection_char_count(&r), 10);
    }

    #[test]
    fn selection_char_count_multi_line() {
        let r = SelectionRange::new(1, 10, 4, 20);
        // first: 80-10=70, middle: 2*80=160, last: 20 => 250
        assert_eq!(selection_char_count(&r), 250);
    }

    #[test]
    fn is_strict_subset_true() {
        let outer = SelectionRange::new(1, 1, 10, 10);
        let inner = SelectionRange::new(2, 2, 5, 5);
        assert!(is_strict_subset(&inner, &outer));
    }

    #[test]
    fn is_strict_subset_equal_is_false() {
        let a = SelectionRange::new(1, 1, 5, 5);
        let b = SelectionRange::new(1, 1, 5, 5);
        assert!(!is_strict_subset(&a, &b));
    }

    #[test]
    fn max_chain_depth_empty() {
        let ranges: Vec<SelectionRange> = vec![];
        assert_eq!(max_chain_depth(&ranges), 0);
    }

    #[test]
    fn max_chain_depth_with_parent() {
        let r1 = SelectionRange::new(1, 1, 1, 5);
        let r2 = SelectionRange::new(1, 1, 1, 5)
            .with_parent(SelectionRange::new(1, 1, 5, 5));
        assert_eq!(max_chain_depth(&[r1, r2]), 1);
    }

    #[test]
    fn flatten_chain_extracts_tuples() {
        let r = SelectionRange::new(1, 1, 1, 5)
            .with_parent(SelectionRange::new(1, 1, 3, 10));
        let flat = flatten_chain(&r);
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0], (1, 1, 1, 5));
        assert_eq!(flat[1], (1, 1, 3, 10));
    }

    #[test]
    fn bounding_range_multiple() {
        let ranges = vec![
            SelectionRange::new(2, 5, 3, 10),
            SelectionRange::new(1, 1, 2, 8),
        ];
        let b = bounding_range(&ranges).unwrap();
        assert_eq!(b.start_line, 1);
        assert_eq!(b.start_col, 1);
        assert_eq!(b.end_line, 3);
        assert_eq!(b.end_col, 10);
    }

    #[test]
    fn bounding_range_empty() {
        assert!(bounding_range(&[]).is_none());
    }

    #[test]
    fn selections_adjacent_true() {
        let a = SelectionRange::new(1, 1, 1, 5);
        let b = SelectionRange::new(1, 5, 1, 10);
        assert!(selections_adjacent(&a, &b));
    }

    #[test]
    fn selections_adjacent_false() {
        let a = SelectionRange::new(1, 1, 1, 5);
        let b = SelectionRange::new(1, 7, 1, 10);
        assert!(!selections_adjacent(&a, &b));
    }

    #[test]
    fn sort_selections_by_position() {
        let mut sels = vec![
            SelectionRange::new(3, 1, 3, 5),
            SelectionRange::new(1, 1, 1, 5),
            SelectionRange::new(2, 5, 2, 10),
        ];
        sort_selections(&mut sels);
        assert_eq!(sels[0].start_line, 1);
        assert_eq!(sels[1].start_line, 2);
        assert_eq!(sels[2].start_line, 3);
    }

    #[test]
    fn dedup_selections_removes_duplicates() {
        let mut sels = vec![
            SelectionRange::new(1, 1, 1, 5),
            SelectionRange::new(1, 1, 1, 5),
            SelectionRange::new(2, 1, 2, 5),
        ];
        dedup_selections(&mut sels);
        assert_eq!(sels.len(), 2);
    }

    #[test]
    fn merge_overlapping_selections_merges() {
        let sels = vec![
            SelectionRange::new(1, 1, 1, 10),
            SelectionRange::new(1, 5, 1, 15),
        ];
        let merged = merge_overlapping_selections(&sels);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start_col, 1);
        assert_eq!(merged[0].end_col, 15);
    }

    #[test]
    fn merge_overlapping_selections_keeps_disjoint() {
        let sels = vec![
            SelectionRange::new(1, 1, 1, 5),
            SelectionRange::new(2, 1, 2, 5),
        ];
        let merged = merge_overlapping_selections(&sels);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn text_char_count_single_line() {
        let range = SelectionRange::new(1, 2, 1, 6);
        let lines = vec!["hello world"];
        assert_eq!(selection_text_char_count(&range, &lines), 4);
    }

    #[test]
    fn text_char_count_multi_line() {
        let range = SelectionRange::new(1, 3, 3, 4);
        let lines = vec!["abcde", "fghij", "klmno"];
        // line 1: from col 3 onwards = "cde" (3 chars)
        // line 2: entire = "fghij" (5 chars)
        // line 3: up to col 4 = "klm" (3 chars)
        assert_eq!(selection_text_char_count(&range, &lines), 11);
    }

    #[test]
    fn extract_selected_text_single_line() {
        let range = SelectionRange::new(1, 1, 1, 6);
        let lines = vec!["hello world"];
        assert_eq!(extract_selected_text(&range, &lines), "hello");
    }

    #[test]
    fn extract_selected_text_multi_line() {
        let range = SelectionRange::new(1, 4, 2, 4);
        let lines = vec!["abcdef", "ghijkl"];
        assert_eq!(extract_selected_text(&range, &lines), "def\nghi");
    }

    #[test]
    fn merge_empty_returns_empty() {
        let merged = merge_overlapping_selections(&[]);
        assert!(merged.is_empty());
    }

    // -- SmartSelectExpander tests --

    #[test]
    fn expander_range_for_level() {
        let entries = vec![
            (ExpansionLevel::Word, SelectionRange::new(1, 1, 1, 5)),
            (ExpansionLevel::Block, SelectionRange::new(1, 1, 5, 20)),
        ];
        let exp = SmartSelectExpander::new(entries);
        assert!(exp.range_for(ExpansionLevel::Word).is_some());
        assert!(exp.range_for(ExpansionLevel::Line).is_none());
        assert!(exp.range_for(ExpansionLevel::Block).is_some());
        assert_eq!(exp.len(), 2);
    }

    #[test]
    fn expander_expand_and_shrink() {
        let entries = vec![
            (ExpansionLevel::Word, SelectionRange::new(1, 1, 1, 5)),
            (ExpansionLevel::Line, SelectionRange::new(1, 1, 1, 30)),
            (ExpansionLevel::Function, SelectionRange::new(1, 1, 20, 1)),
        ];
        let exp = SmartSelectExpander::new(entries);
        let expanded = exp.expand_from(ExpansionLevel::Word).unwrap();
        assert_eq!(expanded.level, ExpansionLevel::Line);
        let shrunk = exp.shrink_from(ExpansionLevel::Line).unwrap();
        assert_eq!(shrunk.level, ExpansionLevel::Word);
    }

    #[test]
    fn expander_no_expand_from_file() {
        let entries = vec![
            (ExpansionLevel::File, SelectionRange::new(1, 1, 100, 1)),
        ];
        let exp = SmartSelectExpander::new(entries);
        assert!(exp.expand_from(ExpansionLevel::File).is_none());
    }

    // -- SmartSelectHistory tests --

    #[test]
    fn history_push_pop() {
        let mut hist = SmartSelectHistory::new();
        assert!(hist.is_empty());
        hist.push(ExpansionLevel::Word, SelectionRange::new(1, 1, 1, 5));
        hist.push(ExpansionLevel::Block, SelectionRange::new(1, 1, 5, 1));
        assert_eq!(hist.len(), 2);
        assert_eq!(hist.current_level(), Some(ExpansionLevel::Block));
        let (lvl, _) = hist.pop().unwrap();
        assert_eq!(lvl, ExpansionLevel::Block);
        assert_eq!(hist.current_level(), Some(ExpansionLevel::Word));
    }

    #[test]
    fn history_clear() {
        let mut hist = SmartSelectHistory::default();
        hist.push(ExpansionLevel::Word, SelectionRange::new(1, 1, 1, 5));
        hist.clear();
        assert!(hist.is_empty());
        assert!(hist.pop().is_none());
    }

    // -- SmartSelectHint tests --

    #[test]
    fn hint_from_word_can_expand() {
        let hint = SmartSelectHint::from_level(ExpansionLevel::Word);
        assert!(hint.can_expand());
        assert_eq!(hint.next_level, Some(ExpansionLevel::Line));
        assert!(hint.label.contains("Line"));
    }

    #[test]
    fn hint_from_file_cannot_expand() {
        let hint = SmartSelectHint::from_level(ExpansionLevel::File);
        assert!(!hint.can_expand());
        assert!(hint.label.contains("broadest"));
    }

    #[test]
    fn hint_display() {
        let hint = SmartSelectHint::from_level(ExpansionLevel::Block);
        let s = format!("{}", hint);
        assert!(s.contains("Function"));
    }

    // -- Bracket-aware selection tests --

    #[test]
    fn bracket_pair_all() {
        assert_eq!(BracketPair::all().len(), 4);
    }

    #[test]
    fn find_bracket_range_parens() {
        let text = "foo(bar(baz))end";
        let result = find_bracket_range(text, 5, BracketPair::PARENS);
        assert_eq!(result, Some((3, 12)));
    }

    #[test]
    fn find_bracket_range_nested() {
        let text = "[a, [b, c], d]";
        let result = find_bracket_range(text, 6, BracketPair::BRACKETS);
        assert_eq!(result, Some((4, 9)));
    }

    #[test]
    fn find_bracket_range_unmatched() {
        let text = "no brackets here";
        assert!(find_bracket_range(text, 5, BracketPair::PARENS).is_none());
    }

    #[test]
    fn find_bracket_range_braces() {
        let text = "fn main() { x }";
        let result = find_bracket_range(text, 13, BracketPair::BRACES);
        assert_eq!(result, Some((10, 14)));
    }

    // -- SmartSelectBracketBalancer tests --------------------------------------

    #[test]
    fn bracket_is_open_close() {
        let b = SmartSelectBracketBalancer::new();
        assert!(b.is_open('('));
        assert!(b.is_open('{'));
        assert!(b.is_close(')'));
        assert!(!b.is_open('a'));
    }

    #[test]
    fn bracket_matching() {
        let b = SmartSelectBracketBalancer::new();
        assert_eq!(b.matching_bracket('('), Some(')'));
        assert_eq!(b.matching_bracket(']'), Some('['));
        assert_eq!(b.matching_bracket('a'), None);
    }

    #[test]
    fn bracket_is_balanced() {
        let b = SmartSelectBracketBalancer::new();
        assert!(b.is_balanced("(hello [world])"));
        assert!(!b.is_balanced("(hello [world)"));
        assert!(b.is_balanced(""));
    }

    #[test]
    fn bracket_expand_at() {
        let b = SmartSelectBracketBalancer::new();
        // text: "(abc)"  positions: 0='(' 1='a' 2='b' 3='c' 4=')'
        let result = b.expand_at("(abc)", 2);
        assert_eq!(result, Some((0, 4)));
    }

    #[test]
    fn bracket_expand_nested() {
        let b = SmartSelectBracketBalancer::new();
        // text: "([x])"  positions: 0='(' 1='[' 2='x' 3=']' 4=')'
        let result = b.expand_at("([x])", 2);
        assert_eq!(result, Some((1, 3)));
    }

    #[test]
    fn bracket_expand_no_match() {
        let b = SmartSelectBracketBalancer::new();
        let result = b.expand_at("hello", 2);
        assert_eq!(result, None);
    }

    #[test]
    fn bracket_find_all_pairs() {
        let b = SmartSelectBracketBalancer::new();
        let pairs = b.find_all_pairs("(a[b]c)");
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn bracket_custom_set() {
        let b = SmartSelectBracketBalancer::with_brackets(vec![BracketDef::new('|', '|')]);
        assert!(b.is_open('|'));
        assert!(b.is_close('|'));
    }

    // -- SmartSelectWordExtender tests ----------------------------------------

    #[test]
    fn word_classify() {
        let w = SmartSelectWordExtender::new();
        assert_eq!(w.classify('a'), CharClass::Word);
        assert_eq!(w.classify(' '), CharClass::Whitespace);
        assert_eq!(w.classify('5'), CharClass::Digit);
        assert_eq!(w.classify('.'), CharClass::Punctuation);
        assert_eq!(w.classify('_'), CharClass::Word);
    }

    #[test]
    fn word_at_basic() {
        let w = SmartSelectWordExtender::new();
        // "hello world" -> word at pos 1 => "hello" -> (0, 5)
        let result = w.word_at("hello world", 1);
        assert_eq!(result, Some((0, 5)));
    }

    #[test]
    fn word_at_second_word() {
        let w = SmartSelectWordExtender::new();
        let result = w.word_at("hello world", 7);
        assert_eq!(result, Some((6, 11)));
    }

    #[test]
    fn word_split_words() {
        let w = SmartSelectWordExtender::new();
        let words = w.split_words("hello world 123");
        assert_eq!(words, vec!["hello", "world", "123"]);
    }

    #[test]
    fn word_extend_with_trailing_space() {
        let w = SmartSelectWordExtender::new();
        let (s, e) = w.extend_with_trailing_space("hello   world", 0, 5);
        assert_eq!((s, e), (0, 8));
    }

    #[test]
    fn word_at_whitespace_returns_none() {
        let w = SmartSelectWordExtender::new();
        let result = w.word_at("hello world", 5);
        assert_eq!(result, None);
    }



    #[test]
    fn smartselect_x_config_new() {
        let c = SmartselectXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn smartselect_x_config_builder() {
        let c = SmartselectXConfig::new("k")
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
    fn smartselect_x_config_display() {
        let c = SmartselectXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn smartselect_x_registry_insert_get() {
        let mut reg = SmartselectXRegistry::new();
        reg.insert(SmartselectXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn smartselect_x_registry_duplicate() {
        let mut reg = SmartselectXRegistry::new();
        reg.insert(SmartselectXConfig::new("a")).unwrap();
        assert!(reg.insert(SmartselectXConfig::new("a")).is_err());
    }

    #[test]
    fn smartselect_x_registry_remove() {
        let mut reg = SmartselectXRegistry::new();
        reg.insert(SmartselectXConfig::new("a")).unwrap();
        reg.insert(SmartselectXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn smartselect_x_registry_active_entries() {
        let mut reg = SmartselectXRegistry::new();
        reg.insert(SmartselectXConfig::new("a")).unwrap();
        reg.insert(SmartselectXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn smartselect_x_registry_by_weight() {
        let mut reg = SmartselectXRegistry::new();
        reg.insert(SmartselectXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(SmartselectXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn smartselect_x_registry_tags() {
        let mut reg = SmartselectXRegistry::new();
        reg.insert(SmartselectXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(SmartselectXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn smartselect_x_registry_total_weight() {
        let mut reg = SmartselectXRegistry::new();
        reg.insert(SmartselectXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(SmartselectXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn smartselect_x_registry_iterator() {
        let mut reg = SmartselectXRegistry::new();
        reg.insert(SmartselectXConfig::new("a")).unwrap();
        reg.insert(SmartselectXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn smartselect_x_cache_put_get() {
        let mut cache = SmartselectXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn smartselect_x_cache_eviction() {
        let mut cache = SmartselectXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn smartselect_x_cache_lru_order() {
        let mut cache = SmartselectXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn smartselect_x_cache_most_least_recent() {
        let mut cache = SmartselectXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn smartselect_x_formatter_entry() {
        let e = SmartselectXConfig::new("k").with_value("v");
        let fmt = SmartselectXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn smartselect_x_formatter_summary() {
        let mut reg = SmartselectXRegistry::new();
        reg.insert(SmartselectXConfig::new("a").with_weight(5)).unwrap();
        let fmt = SmartselectXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn smartselect_x_validator_valid() {
        let v = SmartselectXValidator::new();
        let c = SmartselectXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn smartselect_x_validator_empty_key() {
        let v = SmartselectXValidator::new();
        let c = SmartselectXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn smartselect_x_validator_require_value() {
        let v = SmartselectXValidator::new().require_value(true);
        let c = SmartselectXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn smartselect_x_validator_allowed_tags() {
        let v = SmartselectXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = SmartselectXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn smartselect_x_validator_validate_all() {
        let v = SmartselectXValidator::new();
        let mut reg = SmartselectXRegistry::new();
        reg.insert(SmartselectXConfig::new("ok")).unwrap();
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


    // xa_ extended tests for smartselect
    #[test]
    fn xa_smartselect_ring_new() {
        let rb = super::XaSmartselectRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_smartselect_ring_push_len() {
        let mut rb = super::XaSmartselectRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_smartselect_ring_wrap() {
        let mut rb = super::XaSmartselectRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_smartselect_ring_mean_empty() {
        let rb = super::XaSmartselectRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_smartselect_ring_mean_values() {
        let mut rb = super::XaSmartselectRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_smartselect_ring_min_max() {
        let mut rb = super::XaSmartselectRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_smartselect_ring_iter() {
        let mut rb = super::XaSmartselectRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_smartselect_counter_new() {
        let c = super::XaSmartselectCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_smartselect_counter_inc() {
        let mut c = super::XaSmartselectCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_smartselect_counter_inc_by() {
        let mut c = super::XaSmartselectCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_smartselect_counter_reset() {
        let mut c = super::XaSmartselectCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_smartselect_counter_clear() {
        let mut c = super::XaSmartselectCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_smartselect_counter_default() {
        let c = super::XaSmartselectCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 160 ----

    #[test]
    fn xc_160_pool_new_empty() {
        let pool: super::Xc160Pool<i32> = super::Xc160Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_160_pool_release_acquire() {
        let mut pool = super::Xc160Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_160_pool_acquire_empty() {
        let mut pool: super::Xc160Pool<i32> = super::Xc160Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_160_pool_full() {
        let mut pool = super::Xc160Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_160_pool_drain() {
        let mut pool = super::Xc160Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_160_pool_stats() {
        let mut pool = super::Xc160Pool::new(8);
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
    fn xc_160_pool_clear() {
        let mut pool = super::Xc160Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_160_pool_shrink() {
        let mut pool = super::Xc160Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_160_pool_default() {
        let pool: super::Xc160Pool<String> = super::Xc160Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_160_pool_extend() {
        let mut pool = super::Xc160Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_160_pool_retain() {
        let mut pool = super::Xc160Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_160_scheduler_round_robin() {
        let mut sched = super::Xc160Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_160_scheduler_empty() {
        let mut sched = super::Xc160Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_160_scheduler_reset() {
        let mut sched = super::Xc160Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_160_scheduler_add_remove() {
        let mut sched = super::Xc160Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_160_scheduler_targets() {
        let sched = super::Xc160Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_160_hash_empty() {
        assert_eq!(super::xc_160_hash(b""), 5381);
    }

    #[test]
    fn xc_160_hash_data() {
        let h = super::xc_160_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_160_hash(b"hello"), h);
    }

    #[test]
    fn xc_160_reverse_str() {
        assert_eq!(super::xc_160_reverse("abc"), "cba");
        assert_eq!(super::xc_160_reverse(""), "");
    }


    // --- xd_84 deepening tests ---

    #[test]
    fn xd_84_sm_initial_state() {
        let sm = Xd84StateMachine::new();
        assert_eq!(sm.current_state(), Xd84State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_84_sm_valid_idle_to_running() {
        let mut sm = Xd84StateMachine::new();
        assert!(sm.transition(Xd84State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd84State::Running);
    }

    #[test]
    fn xd_84_sm_valid_running_to_paused() {
        let mut sm = Xd84StateMachine::new();
        sm.transition(Xd84State::Running).unwrap();
        assert!(sm.transition(Xd84State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd84State::Paused);
    }

    #[test]
    fn xd_84_sm_valid_running_to_done() {
        let mut sm = Xd84StateMachine::new();
        sm.transition(Xd84State::Running).unwrap();
        assert!(sm.transition(Xd84State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd84State::Done);
    }

    #[test]
    fn xd_84_sm_valid_paused_to_running() {
        let mut sm = Xd84StateMachine::new();
        sm.transition(Xd84State::Running).unwrap();
        sm.transition(Xd84State::Paused).unwrap();
        assert!(sm.transition(Xd84State::Running).is_ok());
    }

    #[test]
    fn xd_84_sm_valid_done_to_idle() {
        let mut sm = Xd84StateMachine::new();
        sm.transition(Xd84State::Running).unwrap();
        sm.transition(Xd84State::Done).unwrap();
        assert!(sm.transition(Xd84State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd84State::Idle);
    }

    #[test]
    fn xd_84_sm_invalid_idle_to_done() {
        let mut sm = Xd84StateMachine::new();
        assert!(sm.transition(Xd84State::Done).is_err());
    }

    #[test]
    fn xd_84_sm_invalid_idle_to_paused() {
        let mut sm = Xd84StateMachine::new();
        assert!(sm.transition(Xd84State::Paused).is_err());
    }

    #[test]
    fn xd_84_sm_history_tracking() {
        let mut sm = Xd84StateMachine::new();
        sm.transition(Xd84State::Running).unwrap();
        sm.transition(Xd84State::Paused).unwrap();
        sm.transition(Xd84State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd84State::Idle);
        assert_eq!(sm.history()[0].to, Xd84State::Running);
        assert_eq!(sm.history()[1].from, Xd84State::Running);
        assert_eq!(sm.history()[2].to, Xd84State::Done);
    }

    #[test]
    fn xd_84_sm_serialize_deserialize() {
        let mut sm = Xd84StateMachine::new();
        sm.transition(Xd84State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd84StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd84State::Running));
    }

    #[test]
    fn xd_84_sm_deserialize_invalid() {
        assert_eq!(Xd84StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_84_sm_reset() {
        let mut sm = Xd84StateMachine::new();
        sm.transition(Xd84State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd84State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_84_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd84EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd84Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_84_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd84EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd84Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd84Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_84_bus_unsubscribe() {
        let mut bus = Xd84EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_84_event_kind_and_payload() {
        let e = Xd84Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd84Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_84_bus_clear_history() {
        let mut bus = Xd84EventBus::new();
        bus.publish(Xd84Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_84_sm_step_counter_increments() {
        let mut sm = Xd84StateMachine::new();
        sm.transition(Xd84State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd84State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #105 --

    #[test]
    fn xf105_trie_insert_search() {
        let mut t = Xf105Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf105_trie_starts_with() {
        let mut t = Xf105Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf105_trie_remove() {
        let mut t = Xf105Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf105_trie_word_count() {
        let mut t = Xf105Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf105_trie_longest_prefix() {
        let mut t = Xf105Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf105_trie_all_words() {
        let mut t = Xf105Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf105_trie_autocomplete() {
        let mut t = Xf105Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf105_trie_empty_search() {
        let t = Xf105Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf105_bloom_add_contains() {
        let mut bf = Xf105BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf105_bloom_probably_absent() {
        let bf = Xf105BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf105_bloom_false_positive_rate() {
        let mut bf = Xf105BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf105_bloom_clear() {
        let mut bf = Xf105BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf105_bloom_union() {
        let mut a = Xf105BloomFilter::xf_new(512, 2);
        let mut b = Xf105BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf105_bloom_intersection_estimate() {
        let mut a = Xf105BloomFilter::xf_new(512, 2);
        let mut b = Xf105BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf105_bloom_union_size_mismatch() {
        let a = Xf105BloomFilter::xf_new(256, 2);
        let b = Xf105BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh159_skip_insert_contains() {
        let mut sl = super::Xh159SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh159_skip_remove() {
        let mut sl = super::Xh159SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh159_skip_len() {
        let mut sl = super::Xh159SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh159_skip_range_query() {
        let mut sl = super::Xh159SkipList::xh_new(4);
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
    fn xh159_skip_floor_ceiling() {
        let mut sl = super::Xh159SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh159_skip_rank() {
        let mut sl = super::Xh159SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh159_skip_empty() {
        let sl = super::Xh159SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh159_skip_duplicates() {
        let mut sl = super::Xh159SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh159_bitset_set_test() {
        let mut bs = super::Xh159BitSet::xh_new(256);
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
    fn xh159_bitset_clear_count() {
        let mut bs = super::Xh159BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh159_bitset_and_or_xor() {
        let mut a = super::Xh159BitSet::xh_new(128);
        let mut b = super::Xh159BitSet::xh_new(128);
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
    fn xh159_bitset_iter_ones() {
        let mut bs = super::Xh159BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh159_bitset_first_last() {
        let mut bs = super::Xh159BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh159_bitset_empty() {
        let bs = super::Xh159BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi159_deque_push_pop_back() {
        let mut dq = super::Xi159Deque::xi_new(4);
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
    fn xi159_deque_push_pop_front() {
        let mut dq = super::Xi159Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi159_deque_mixed_ops() {
        let mut dq = super::Xi159Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi159_deque_get_and_split() {
        let mut dq = super::Xi159Deque::xi_new(8);
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
    fn xi159_deque_rotate_left() {
        let mut dq = super::Xi159Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi159_deque_rotate_right() {
        let mut dq = super::Xi159Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi159_deque_grow() {
        let mut dq = super::Xi159Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi159_deque_empty() {
        let dq = super::Xi159Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi159_interval_tree_insert_query() {
        let mut tree = super::Xi159IntervalTree::xi_new();
        tree.xi_insert(super::Xi159Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi159Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi159Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi159_interval_tree_overlap() {
        let mut tree = super::Xi159IntervalTree::xi_new();
        tree.xi_insert(super::Xi159Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi159Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi159Interval::xi_new(12, 20));
        let q = super::Xi159Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi159_interval_tree_remove() {
        let mut tree = super::Xi159IntervalTree::xi_new();
        tree.xi_insert(super::Xi159Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi159Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi159_interval_tree_gaps() {
        let mut tree = super::Xi159IntervalTree::xi_new();
        tree.xi_insert(super::Xi159Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi159Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi159Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi159Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi159Interval::xi_new(8, 10));
    }

    #[test]
    fn xi159_interval_tree_merge() {
        let mut tree = super::Xi159IntervalTree::xi_new();
        tree.xi_insert(super::Xi159Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi159Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi159Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi159Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi159Interval::xi_new(10, 15));
    }

    #[test]
    fn xi159_interval_tree_all() {
        let mut tree = super::Xi159IntervalTree::xi_new();
        tree.xi_insert(super::Xi159Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi159Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi159_interval_tree_empty() {
        let tree = super::Xi159IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi159_interval_tree_contains_point() {
        let iv = super::Xi159Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 159) ---

    #[test]
    fn xj_159_uf_make_and_find() {
        let mut uf = super::Xj159UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_159_uf_union_connected() {
        let mut uf = super::Xj159UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_159_uf_component_count() {
        let mut uf = super::Xj159UnionFind::xj_new();
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
    fn xj_159_uf_component_size() {
        let mut uf = super::Xj159UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_159_uf_largest_component() {
        let mut uf = super::Xj159UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_159_uf_many_elements() {
        let mut uf = super::Xj159UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_159_uf_separate_components() {
        let mut uf = super::Xj159UnionFind::xj_new();
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
    fn xj_159_uf_path_compression() {
        let mut uf = super::Xj159UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_159_bt_insert_get() {
        let mut bt = super::Xj159BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_159_bt_contains_len() {
        let mut bt = super::Xj159BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_159_bt_replace() {
        let mut bt = super::Xj159BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_159_bt_remove() {
        let mut bt = super::Xj159BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_159_bt_keys_values() {
        let mut bt = super::Xj159BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_159_bt_range() {
        let mut bt = super::Xj159BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_159_bt_min_max() {
        let mut bt = super::Xj159BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_159_bt_many_inserts() {
        let mut bt = super::Xj159BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_159 segment tree tests ---

    #[test]
    fn xk_159_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk159SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_159_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk159SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_159_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk159SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_159_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk159SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_159_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk159SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_159_st_single_element() {
        let data = vec![42];
        let st = super::Xk159SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_159_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk159SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_159_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk159SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_159 disjoint intervals tests ---

    #[test]
    fn xk_159_di_add_and_count() {
        let mut di = super::Xk159DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_159_di_merge_overlap() {
        let mut di = super::Xk159DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_159_di_contains() {
        let mut di = super::Xk159DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_159_di_remove() {
        let mut di = super::Xk159DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_159_di_covered_length() {
        let mut di = super::Xk159DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_159_di_gaps() {
        let mut di = super::Xk159DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_159_di_merge_adjacent() {
        let mut di = super::Xk159DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_159_di_empty() {
        let di = super::Xk159DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_159_rope_new_empty() {
        let rope = super::Xl159Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_159_rope_from_str() {
        let rope = super::Xl159Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_159_rope_insert_at() {
        let mut rope = super::Xl159Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_159_rope_delete_range() {
        let mut rope = super::Xl159Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_159_rope_char_at() {
        let rope = super::Xl159Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_159_rope_split_concat() {
        let rope = super::Xl159Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_159_rope_line_count() {
        let rope = super::Xl159Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_159_rope_line_at() {
        let rope = super::Xl159Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_159_sa_build_and_search() {
        let sa = super::Xl159SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_159_sa_count() {
        let sa = super::Xl159SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_159_sa_longest_repeated() {
        let sa = super::Xl159SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_159_sa_all_positions() {
        let sa = super::Xl159SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_159_sa_len() {
        let sa = super::Xl159SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_159_sa_empty() {
        let sa = super::Xl159SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_159_rope_slice() {
        let rope = super::Xl159Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_159_sa_search_start() {
        let sa = super::Xl159SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_159_sparse_set_get() {
        let mut m = super::Xm159MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_159_sparse_row_col() {
        let mut m = super::Xm159MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_159_sparse_transpose() {
        let mut m = super::Xm159MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_159_sparse_multiply_vec() {
        let mut m = super::Xm159MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_159_sparse_nnz_density() {
        let mut m = super::Xm159MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_159_sparse_clear() {
        let mut m = super::Xm159MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_159_sparse_overwrite_zero() {
        let mut m = super::Xm159MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_159_tokenizer_basic() {
        let t = super::Xm159Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_159_tokenizer_count() {
        let t = super::Xm159Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_159_tokenizer_unique() {
        let t = super::Xm159Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_159_tokenizer_frequency() {
        let t = super::Xm159Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_159_tokenizer_delimiter() {
        let t = super::Xm159Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_159_tokenizer_whitespace() {
        let t = super::Xm159Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_159_tokenizer_empty() {
        let t = super::Xm159Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 159 ----

    #[test]
    fn xn_159_fenwick_prefix_sum() {
        let mut ft = super::Xn159Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_159_fenwick_range_sum() {
        let mut ft = super::Xn159Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_159_fenwick_point_query() {
        let mut ft = super::Xn159Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_159_fenwick_len() {
        let ft = super::Xn159Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_159_fenwick_multiple_updates() {
        let mut ft = super::Xn159Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_159_fenwick_single_element() {
        let mut ft = super::Xn159Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_159_fenwick_find_kth() {
        let mut ft = super::Xn159Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_159_fenwick_negative_delta() {
        let mut ft = super::Xn159Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 159 ----

    #[test]
    fn xn_159_avl_insert_get() {
        let mut m = super::Xn159AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_159_avl_remove() {
        let mut m = super::Xn159AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_159_avl_in_order() {
        let mut m = super::Xn159AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_159_avl_min_max() {
        let mut m = super::Xn159AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_159_avl_floor_ceiling() {
        let mut m = super::Xn159AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_159_avl_height_balanced() {
        let mut m = super::Xn159AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_159_avl_overwrite() {
        let mut m = super::Xn159AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_159_avl_empty() {
        let m: super::Xn159AVL<i32, i32> = super::Xn159AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
