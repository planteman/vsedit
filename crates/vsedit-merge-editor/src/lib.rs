//! 3-way merge editor.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during merge operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// Conflict index is out of bounds.
    ConflictOutOfRange { index: usize, total: usize },
    /// Attempted to produce a merged result with unresolved conflicts.
    UnresolvedConflicts { remaining: usize },
    /// A conflict region has invalid line ranges.
    InvalidRegion { start: u32, end: u32 },
    /// Custom resolution text was empty.
    EmptyCustomResolution,
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::ConflictOutOfRange { index, total } => {
                write!(f, "conflict index {index} out of range (total {total})")
            }
            MergeError::UnresolvedConflicts { remaining } => {
                write!(f, "{remaining} conflict(s) still unresolved")
            }
            MergeError::InvalidRegion { start, end } => {
                write!(f, "invalid conflict region: start {start} >= end {end}")
            }
            MergeError::EmptyCustomResolution => {
                write!(f, "custom resolution text must not be empty")
            }
        }
    }
}

impl std::error::Error for MergeError {}

/// Which side of a merge conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictSide {
    Current,
    Incoming,
    Base,
}

impl fmt::Display for ConflictSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictSide::Current => write!(f, "Current"),
            ConflictSide::Incoming => write!(f, "Incoming"),
            ConflictSide::Base => write!(f, "Base"),
        }
    }
}

/// A single merge conflict region.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeConflict {
    pub base_start: u32,
    pub base_end: u32,
    pub current_text: String,
    pub incoming_text: String,
    pub base_text: String,
    pub resolved: bool,
    pub resolution: Option<String>,
}

impl MergeConflict {
    /// Validate that this conflict has a sane line region.
    pub fn validate(&self) -> Result<(), MergeError> {
        if self.base_start >= self.base_end {
            return Err(MergeError::InvalidRegion {
                start: self.base_start,
                end: self.base_end,
            });
        }
        Ok(())
    }

    /// Number of lines this conflict region spans.
    pub fn line_span(&self) -> u32 {
        self.base_end.saturating_sub(self.base_start)
    }

    /// True when current and incoming sides are identical (trivially resolvable).
    pub fn is_trivial(&self) -> bool {
        self.current_text == self.incoming_text
    }

    /// Return the text for a given side.
    pub fn text_for_side(&self, side: ConflictSide) -> &str {
        match side {
            ConflictSide::Current => &self.current_text,
            ConflictSide::Incoming => &self.incoming_text,
            ConflictSide::Base => &self.base_text,
        }
    }

    /// Final resolved text, falling back to `base_text` if unresolved.
    pub fn resolved_text(&self) -> &str {
        self.resolution.as_deref().unwrap_or(&self.base_text)
    }
}

impl fmt::Display for MergeConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.resolved { "resolved" } else { "unresolved" };
        write!(
            f,
            "MergeConflict(lines {}..{}, {})",
            self.base_start, self.base_end, status
        )
    }
}

/// Builder for constructing a `MergeConflict`.
#[derive(Debug, Clone, Default)]
pub struct MergeConflictBuilder {
    base_start: u32,
    base_end: u32,
    current_text: String,
    incoming_text: String,
    base_text: String,
}

impl MergeConflictBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn region(mut self, start: u32, end: u32) -> Self {
        self.base_start = start;
        self.base_end = end;
        self
    }

    pub fn current_text(mut self, text: impl Into<String>) -> Self {
        self.current_text = text.into();
        self
    }

    pub fn incoming_text(mut self, text: impl Into<String>) -> Self {
        self.incoming_text = text.into();
        self
    }

    pub fn base_text(mut self, text: impl Into<String>) -> Self {
        self.base_text = text.into();
        self
    }

    /// Build the conflict, returning an error if the region is invalid.
    pub fn build(self) -> Result<MergeConflict, MergeError> {
        let conflict = MergeConflict {
            base_start: self.base_start,
            base_end: self.base_end,
            current_text: self.current_text,
            incoming_text: self.incoming_text,
            base_text: self.base_text,
            resolved: false,
            resolution: None,
        };
        conflict.validate()?;
        Ok(conflict)
    }
}

/// How a conflict should be resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum MergeResolution {
    AcceptCurrent,
    AcceptIncoming,
    AcceptBoth,
    Custom(String),
}

impl fmt::Display for MergeResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeResolution::AcceptCurrent => write!(f, "Accept Current"),
            MergeResolution::AcceptIncoming => write!(f, "Accept Incoming"),
            MergeResolution::AcceptBoth => write!(f, "Accept Both"),
            MergeResolution::Custom(_) => write!(f, "Custom"),
        }
    }
}

/// Display mode for the merge editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeEditorMode {
    SideBySide,
    Inline,
}

impl fmt::Display for MergeEditorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeEditorMode::SideBySide => write!(f, "Side-by-Side"),
            MergeEditorMode::Inline => write!(f, "Inline"),
        }
    }
}

/// Widget that manages merge conflicts.
#[derive(Clone, PartialEq)]
pub struct MergeEditorWidget {
    pub conflicts: Vec<MergeConflict>,
    pub mode: MergeEditorMode,
    pub current_conflict: usize,
}

impl fmt::Debug for MergeEditorWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MergeEditorWidget")
            .field("num_conflicts", &self.conflicts.len())
            .field("mode", &self.mode)
            .field("current_conflict", &self.current_conflict)
            .finish()
    }
}

impl fmt::Display for MergeEditorWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let resolved = self.resolved_count();
        let total = self.conflicts.len();
        write!(f, "MergeEditor({resolved}/{total} resolved, {})", self.mode)
    }
}

impl MergeEditorWidget {
    pub fn new() -> Self {
        Self {
            conflicts: Vec::new(),
            mode: MergeEditorMode::Inline,
            current_conflict: 0,
        }
    }

    pub fn add_conflict(&mut self, conflict: MergeConflict) {
        self.conflicts.push(conflict);
    }

    pub fn resolve_conflict(&mut self, index: usize, resolution: MergeResolution) {
        if let Some(c) = self.conflicts.get_mut(index) {
            c.resolution = Some(match &resolution {
                MergeResolution::AcceptCurrent => c.current_text.clone(),
                MergeResolution::AcceptIncoming => c.incoming_text.clone(),
                MergeResolution::AcceptBoth => {
                    let mut s = c.current_text.clone();
                    if !s.is_empty() && !c.incoming_text.is_empty() {
                        s.push('\n');
                    }
                    s.push_str(&c.incoming_text);
                    s
                }
                MergeResolution::Custom(text) => text.clone(),
            });
            c.resolved = true;
        }
    }

    pub fn next_conflict(&mut self) {
        if !self.conflicts.is_empty() && self.current_conflict + 1 < self.conflicts.len() {
            self.current_conflict += 1;
        }
    }

    pub fn prev_conflict(&mut self) {
        if self.current_conflict > 0 {
            self.current_conflict -= 1;
        }
    }

    pub fn all_resolved(&self) -> bool {
        !self.conflicts.is_empty() && self.conflicts.iter().all(|c| c.resolved)
    }

    pub fn get_merged_result(&self) -> Vec<String> {
        self.conflicts
            .iter()
            .map(|c| {
                if let Some(ref res) = c.resolution {
                    res.clone()
                } else {
                    c.base_text.clone()
                }
            })
            .collect()
    }

    /// Resolve the current conflict and advance to the next unresolved one.
    pub fn resolve_current(&mut self, resolution: MergeResolution) {
        let idx = self.current_conflict;
        self.resolve_conflict(idx, resolution);
        self.jump_to_next_unresolved();
    }

    /// Checked variant of `resolve_conflict` that returns `MergeError`.
    pub fn try_resolve_conflict(
        &mut self,
        index: usize,
        resolution: MergeResolution,
    ) -> Result<(), MergeError> {
        if index >= self.conflicts.len() {
            return Err(MergeError::ConflictOutOfRange {
                index,
                total: self.conflicts.len(),
            });
        }
        if let MergeResolution::Custom(ref t) = resolution {
            if t.is_empty() {
                return Err(MergeError::EmptyCustomResolution);
            }
        }
        self.resolve_conflict(index, resolution);
        Ok(())
    }

    /// Produce merged output only when every conflict is resolved.
    pub fn try_get_merged_result(&self) -> Result<Vec<String>, MergeError> {
        let remaining = self.unresolved_count();
        if remaining > 0 {
            return Err(MergeError::UnresolvedConflicts { remaining });
        }
        Ok(self.get_merged_result())
    }

    /// Number of resolved conflicts.
    pub fn resolved_count(&self) -> usize {
        self.conflicts.iter().filter(|c| c.resolved).count()
    }

    /// Number of unresolved conflicts.
    pub fn unresolved_count(&self) -> usize {
        self.conflicts.iter().filter(|c| !c.resolved).count()
    }

    /// Automatically resolve any trivial conflicts (identical current/incoming).
    pub fn auto_resolve_trivial(&mut self) -> usize {
        let mut count = 0;
        for c in &mut self.conflicts {
            if !c.resolved && c.is_trivial() {
                c.resolution = Some(c.current_text.clone());
                c.resolved = true;
                count += 1;
            }
        }
        count
    }

    /// Jump to the next unresolved conflict after current position.
    pub fn jump_to_next_unresolved(&mut self) {
        let start = self.current_conflict + 1;
        for i in start..self.conflicts.len() {
            if !self.conflicts[i].resolved {
                self.current_conflict = i;
                return;
            }
        }
        // Wrap around from the beginning.
        for i in 0..start.min(self.conflicts.len()) {
            if !self.conflicts[i].resolved {
                self.current_conflict = i;
                return;
            }
        }
    }

    /// Toggle between `Inline` and `SideBySide` modes.
    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            MergeEditorMode::Inline => MergeEditorMode::SideBySide,
            MergeEditorMode::SideBySide => MergeEditorMode::Inline,
        };
    }

    /// Return a reference to the currently selected conflict, if any.
    pub fn current(&self) -> Option<&MergeConflict> {
        self.conflicts.get(self.current_conflict)
    }
}

impl Default for MergeEditorWidget {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse standard conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`) from text.
pub fn parse_conflict_markers(text: &str) -> Vec<MergeConflict> {
    let mut conflicts = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        if lines[i].starts_with("<<<<<<<") {
            let start = i as u32;
            let mut current_lines = Vec::new();
            let mut base_lines: Vec<&str> = Vec::new();
            let mut incoming_lines = Vec::new();
            i += 1;

            // Collect current (ours) side, watching for optional ||||||| base marker
            let mut has_base = false;
            while i < lines.len() && !lines[i].starts_with("=======") && !lines[i].starts_with("|||||||") {
                current_lines.push(lines[i]);
                i += 1;
            }

            // Optional base section (diff3 style)
            if i < lines.len() && lines[i].starts_with("|||||||") {
                has_base = true;
                i += 1;
                while i < lines.len() && !lines[i].starts_with("=======") {
                    base_lines.push(lines[i]);
                    i += 1;
                }
            }

            // Skip =======
            if i < lines.len() && lines[i].starts_with("=======") {
                i += 1;
            }

            // Collect incoming (theirs) side
            while i < lines.len() && !lines[i].starts_with(">>>>>>>") {
                incoming_lines.push(lines[i]);
                i += 1;
            }

            let end = i as u32;

            let base_text = if has_base {
                base_lines.join("\n")
            } else {
                String::new()
            };

            conflicts.push(MergeConflict {
                base_start: start,
                base_end: end,
                current_text: current_lines.join("\n"),
                incoming_text: incoming_lines.join("\n"),
                base_text,
                resolved: false,
                resolution: None,
            });
        }
        i += 1;
    }

    conflicts
}

/// Statistics about the conflicts in a merge editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictStats {
    pub total: usize,
    pub resolved: usize,
    pub unresolved: usize,
    pub trivial: usize,
    pub total_lines: u32,
}

impl MergeEditorWidget {
    /// Compute detailed statistics about the conflicts.
    pub fn conflict_stats(&self) -> ConflictStats {
        let total = self.conflicts.len();
        let resolved = self.resolved_count();
        let trivial = self.conflicts.iter().filter(|c| c.is_trivial()).count();
        let total_lines: u32 = self.conflicts.iter().map(|c| c.line_span()).sum();
        ConflictStats {
            total,
            resolved,
            unresolved: total - resolved,
            trivial,
            total_lines,
        }
    }

    /// Validate the merged result: check that all resolutions are non-empty.
    pub fn validate_result(&self) -> Result<(), MergeError> {
        let remaining = self.unresolved_count();
        if remaining > 0 {
            return Err(MergeError::UnresolvedConflicts { remaining });
        }
        for c in &self.conflicts {
            if let Some(ref res) = c.resolution {
                if res.is_empty() {
                    return Err(MergeError::EmptyCustomResolution);
                }
            }
        }
        Ok(())
    }

    /// Auto-resolve conflicts where changes don't overlap with each other.
    /// A conflict is considered non-overlapping if its current text equals the base text
    /// (only incoming changed) or its incoming text equals the base text (only current changed).
    pub fn auto_resolve_non_overlapping(&mut self) -> usize {
        let mut count = 0;
        for c in &mut self.conflicts {
            if c.resolved {
                continue;
            }
            if c.current_text == c.base_text && c.incoming_text != c.base_text {
                c.resolution = Some(c.incoming_text.clone());
                c.resolved = true;
                count += 1;
            } else if c.incoming_text == c.base_text && c.current_text != c.base_text {
                c.resolution = Some(c.current_text.clone());
                c.resolved = true;
                count += 1;
            }
        }
        count
    }

    /// Generate a preview of the merge result, showing conflict markers for unresolved conflicts.
    pub fn generate_preview(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for c in &self.conflicts {
            if c.resolved {
                if let Some(ref res) = c.resolution {
                    lines.push(res.clone());
                }
            } else {
                lines.push(format!("<<<<<<< Current"));
                lines.push(c.current_text.clone());
                lines.push("=======".to_string());
                lines.push(c.incoming_text.clone());
                lines.push(format!(">>>>>>> Incoming"));
            }
        }
        lines
    }

    /// Return indices of all unresolved conflicts.
    pub fn unresolved_indices(&self) -> Vec<usize> {
        self.conflicts
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.resolved)
            .map(|(i, _)| i)
            .collect()
    }
}

impl fmt::Display for ConflictStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ConflictStats(total={}, resolved={}, unresolved={}, trivial={}, lines={})",
            self.total, self.resolved, self.unresolved, self.trivial, self.total_lines
        )
    }
}

/// A region where the base, ours, and theirs versions conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictRegion {
    /// Start index in the base (inclusive).
    pub base_start: usize,
    /// End index in the base (exclusive).
    pub base_end: usize,
    /// Lines from the "ours" side for this region.
    pub ours_lines: Vec<String>,
    /// Lines from the "theirs" side for this region.
    pub theirs_lines: Vec<String>,
}

/// 3-way merge inputs.
#[derive(Debug, Clone)]
pub struct ThreeWayMerge {
    pub base: Vec<String>,
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
}

impl ThreeWayMerge {
    pub fn new(base: Vec<String>, ours: Vec<String>, theirs: Vec<String>) -> Self {
        Self { base, ours, theirs }
    }

    /// Returns `true` when ours and theirs disagree and at least one differs from base.
    pub fn has_conflicts(&self) -> bool {
        if self.ours == self.theirs {
            return false;
        }
        let max_len = self.base.len().max(self.ours.len()).max(self.theirs.len());
        for i in 0..max_len {
            let b = self.base.get(i);
            let o = self.ours.get(i);
            let t = self.theirs.get(i);
            if o != t && (o != b || t != b) {
                return true;
            }
        }
        false
    }

    pub fn base_line_count(&self) -> usize {
        self.base.len()
    }

    pub fn ours_line_count(&self) -> usize {
        self.ours.len()
    }

    pub fn theirs_line_count(&self) -> usize {
        self.theirs.len()
    }
}

/// Finds non-overlapping conflict regions by comparing line-by-line.
///
/// When both ours and theirs differ from base at a given position, the line
/// belongs to a conflict region. Adjacent conflict lines are merged into a
/// single [`ConflictRegion`].
pub fn conflict_regions(
    base: &[String],
    ours: &[String],
    theirs: &[String],
) -> Vec<ConflictRegion> {
    let max_len = base.len().max(ours.len()).max(theirs.len());
    let mut regions: Vec<ConflictRegion> = Vec::new();

    let mut i = 0;
    while i < max_len {
        let b = base.get(i);
        let o = ours.get(i);
        let t = theirs.get(i);

        let ours_differs = o != b;
        let theirs_differs = t != b;

        if ours_differs && theirs_differs && o != t {
            // Start of a conflict region.
            let start = i;
            let mut o_lines = Vec::new();
            let mut t_lines = Vec::new();
            while i < max_len {
                let b2 = base.get(i);
                let o2 = ours.get(i);
                let t2 = theirs.get(i);
                let od = o2 != b2;
                let td = t2 != b2;
                if od && td && o2 != t2 {
                    if let Some(o2) = o2 {
                        o_lines.push(o2.clone());
                    }
                    if let Some(t2) = t2 {
                        t_lines.push(t2.clone());
                    }
                    i += 1;
                } else {
                    break;
                }
            }
            regions.push(ConflictRegion {
                base_start: start,
                base_end: i,
                ours_lines: o_lines,
                theirs_lines: t_lines,
            });
        } else {
            i += 1;
        }
    }

    regions
}

/// Result of an automatic three-way merge attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoResolveResult {
    pub merged_lines: Vec<String>,
    pub had_conflicts: bool,
    pub conflict_count: usize,
}

/// Attempts to automatically merge three versions line-by-line.
///
/// * If ours == theirs, uses ours.
/// * If only ours differs from base, uses ours.
/// * If only theirs differs from base, uses theirs.
/// * If both differ from base and from each other, inserts conflict markers.
pub fn auto_resolve(
    base: &[String],
    ours: &[String],
    theirs: &[String],
) -> AutoResolveResult {
    let max_len = base.len().max(ours.len()).max(theirs.len());
    let mut merged = Vec::new();
    let mut conflict_count: usize = 0;

    let mut i = 0;
    while i < max_len {
        let b = base.get(i).map(|s| s.as_str());
        let o = ours.get(i).map(|s| s.as_str());
        let t = theirs.get(i).map(|s| s.as_str());

        if o == t {
            // Both sides agree — use whichever is present (prefer ours).
            if let Some(line) = o {
                merged.push(line.to_string());
            }
        } else {
            let ours_differs = o != b;
            let theirs_differs = t != b;

            if ours_differs && theirs_differs {
                // True conflict — collect consecutive conflicting lines.
                conflict_count += 1;
                let mut o_lines: Vec<&str> = Vec::new();
                let mut t_lines: Vec<&str> = Vec::new();
                while i < max_len {
                    let b2 = base.get(i).map(|s| s.as_str());
                    let o2 = ours.get(i).map(|s| s.as_str());
                    let t2 = theirs.get(i).map(|s| s.as_str());
                    if o2 != t2 && o2 != b2 && t2 != b2 {
                        if let Some(l) = o2 {
                            o_lines.push(l);
                        }
                        if let Some(l) = t2 {
                            t_lines.push(l);
                        }
                        i += 1;
                    } else {
                        break;
                    }
                }
                merged.push("<<<<<<< ours".to_string());
                for l in &o_lines {
                    merged.push(l.to_string());
                }
                merged.push("=======".to_string());
                for l in &t_lines {
                    merged.push(l.to_string());
                }
                merged.push(">>>>>>> theirs".to_string());
                continue; // i already advanced
            } else if ours_differs {
                if let Some(line) = o {
                    merged.push(line.to_string());
                }
            } else if let Some(line) = t {
                merged.push(line.to_string());
            }
        }
        i += 1;
    }

    AutoResolveResult {
        merged_lines: merged,
        had_conflicts: conflict_count > 0,
        conflict_count,
    }
}

// ---------------------------------------------------------------------------
// Merge conflict statistics by side
// ---------------------------------------------------------------------------

/// Breakdown of conflicts by which side changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictsBySource {
    /// Conflicts where only the current branch changed.
    pub current_only: usize,
    /// Conflicts where only the incoming branch changed.
    pub incoming_only: usize,
    /// Conflicts where both sides changed.
    pub both_changed: usize,
    /// Trivial conflicts (both sides identical).
    pub trivial: usize,
}

impl ConflictsBySource {
    /// Compute from a slice of merge conflicts.
    pub fn from_conflicts(conflicts: &[MergeConflict]) -> Self {
        let mut current_only = 0;
        let mut incoming_only = 0;
        let mut both_changed = 0;
        let mut trivial = 0;
        for c in conflicts {
            if c.is_trivial() {
                trivial += 1;
            } else if c.current_text == c.base_text {
                incoming_only += 1;
            } else if c.incoming_text == c.base_text {
                current_only += 1;
            } else {
                both_changed += 1;
            }
        }
        Self { current_only, incoming_only, both_changed, trivial }
    }
}

impl fmt::Display for ConflictsBySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ConflictsBySource(current={}, incoming={}, both={}, trivial={})",
            self.current_only, self.incoming_only, self.both_changed, self.trivial
        )
    }
}

impl MergeEditorWidget {
    /// Return breakdown of conflicts by source side.
    pub fn conflicts_by_source(&self) -> ConflictsBySource {
        ConflictsBySource::from_conflicts(&self.conflicts)
    }

    /// The resolution ratio as a fraction in [0.0, 1.0].
    pub fn resolution_ratio(&self) -> f64 {
        if self.conflicts.is_empty() {
            return 1.0;
        }
        self.resolved_count() as f64 / self.conflicts.len() as f64
    }

    /// Total number of affected lines across all conflicts.
    pub fn total_affected_lines(&self) -> u32 {
        self.conflicts.iter().map(|c| c.line_span()).sum()
    }
}

// ---------------------------------------------------------------------------
// MergeSession — tracks multiple files being merged
// ---------------------------------------------------------------------------

/// Status of a file within a merge session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeFileStatus {
    Pending,
    InProgress,
    Resolved,
    Skipped,
}

impl fmt::Display for MergeFileStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeFileStatus::Pending => write!(f, "Pending"),
            MergeFileStatus::InProgress => write!(f, "InProgress"),
            MergeFileStatus::Resolved => write!(f, "Resolved"),
            MergeFileStatus::Skipped => write!(f, "Skipped"),
        }
    }
}

/// A file entry in a merge session.
#[derive(Debug, Clone)]
pub struct MergeFileEntry {
    pub path: String,
    pub status: MergeFileStatus,
    pub editor: MergeEditorWidget,
}

impl MergeFileEntry {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            status: MergeFileStatus::Pending,
            editor: MergeEditorWidget::new(),
        }
    }

    /// Mark this entry as resolved if all conflicts are resolved.
    pub fn try_finish(&mut self) -> bool {
        if self.editor.all_resolved() {
            self.status = MergeFileStatus::Resolved;
            true
        } else {
            false
        }
    }
}

/// A session that tracks multiple files being merged.
#[derive(Debug, Clone)]
pub struct MergeSession {
    pub files: Vec<MergeFileEntry>,
    pub current_file: usize,
}

impl MergeSession {
    pub fn new() -> Self {
        Self { files: Vec::new(), current_file: 0 }
    }

    /// Add a file to this merge session.
    pub fn add_file(&mut self, path: impl Into<String>) -> usize {
        let idx = self.files.len();
        self.files.push(MergeFileEntry::new(path));
        idx
    }

    /// Get the current file entry, if any.
    pub fn current_entry(&self) -> Option<&MergeFileEntry> {
        self.files.get(self.current_file)
    }

    /// Get a mutable reference to the current file entry.
    pub fn current_entry_mut(&mut self) -> Option<&mut MergeFileEntry> {
        self.files.get_mut(self.current_file)
    }

    /// Advance to the next file.
    pub fn next_file(&mut self) -> bool {
        if self.current_file + 1 < self.files.len() {
            self.current_file += 1;
            true
        } else {
            false
        }
    }

    /// Go back to the previous file.
    pub fn prev_file(&mut self) -> bool {
        if self.current_file > 0 {
            self.current_file -= 1;
            true
        } else {
            false
        }
    }

    /// Number of files in the session.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Number of fully resolved files.
    pub fn resolved_file_count(&self) -> usize {
        self.files.iter().filter(|f| f.status == MergeFileStatus::Resolved).count()
    }

    /// Number of files still pending or in progress.
    pub fn pending_file_count(&self) -> usize {
        self.files.iter().filter(|f| {
            f.status == MergeFileStatus::Pending || f.status == MergeFileStatus::InProgress
        }).count()
    }

    /// Overall session progress as a fraction in [0.0, 1.0].
    pub fn progress(&self) -> f64 {
        if self.files.is_empty() {
            return 1.0;
        }
        let done = self.files.iter().filter(|f| {
            f.status == MergeFileStatus::Resolved || f.status == MergeFileStatus::Skipped
        }).count();
        done as f64 / self.files.len() as f64
    }

    /// Skip the current file and advance.
    pub fn skip_current(&mut self) {
        if let Some(entry) = self.files.get_mut(self.current_file) {
            entry.status = MergeFileStatus::Skipped;
        }
        self.next_file();
    }

    /// Check whether the entire session is complete.
    pub fn is_complete(&self) -> bool {
        self.files.iter().all(|f| {
            f.status == MergeFileStatus::Resolved || f.status == MergeFileStatus::Skipped
        })
    }
}

impl Default for MergeSession {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MergeSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MergeSession({}/{} resolved, file {}/{})",
            self.resolved_file_count(),
            self.file_count(),
            self.current_file + 1,
            self.file_count()
        )
    }
}

// ---------------------------------------------------------------------------
// Serialization of merge results
// ---------------------------------------------------------------------------

/// Serializable representation of a merge result for a single file.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeResultRecord {
    pub path: String,
    pub status: MergeFileStatus,
    pub total_conflicts: usize,
    pub resolved_conflicts: usize,
    pub merged_lines: Vec<String>,
}

impl MergeResultRecord {
    /// Create from a merge file entry.
    pub fn from_entry(entry: &MergeFileEntry) -> Self {
        Self {
            path: entry.path.clone(),
            status: entry.status.clone(),
            total_conflicts: entry.editor.conflicts.len(),
            resolved_conflicts: entry.editor.resolved_count(),
            merged_lines: entry.editor.get_merged_result(),
        }
    }

    /// The merged content as a single string.
    pub fn merged_text(&self) -> String {
        self.merged_lines.join("\n")
    }

    /// Whether this file was fully resolved.
    pub fn is_fully_resolved(&self) -> bool {
        self.total_conflicts == self.resolved_conflicts && self.total_conflicts > 0
    }
}

impl fmt::Display for MergeResultRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MergeResult({}: {}/{} conflicts resolved, status={})",
            self.path, self.resolved_conflicts, self.total_conflicts, self.status
        )
    }
}

/// Serialize the entire session into result records.
pub fn serialize_session_results(session: &MergeSession) -> Vec<MergeResultRecord> {
    session.files.iter().map(MergeResultRecord::from_entry).collect()
}

/// Summary of a full merge session.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeSessionSummary {
    pub total_files: usize,
    pub resolved_files: usize,
    pub skipped_files: usize,
    pub total_conflicts: usize,
    pub resolved_conflicts: usize,
}

impl MergeSessionSummary {
    pub fn from_session(session: &MergeSession) -> Self {
        let mut total_conflicts = 0;
        let mut resolved_conflicts = 0;
        let mut skipped = 0;
        let mut resolved_files = 0;
        for f in &session.files {
            total_conflicts += f.editor.conflicts.len();
            resolved_conflicts += f.editor.resolved_count();
            if f.status == MergeFileStatus::Resolved {
                resolved_files += 1;
            }
            if f.status == MergeFileStatus::Skipped {
                skipped += 1;
            }
        }
        Self {
            total_files: session.files.len(),
            resolved_files,
            skipped_files: skipped,
            total_conflicts,
            resolved_conflicts,
        }
    }

    /// Overall conflict resolution ratio.
    pub fn conflict_resolution_ratio(&self) -> f64 {
        if self.total_conflicts == 0 {
            return 1.0;
        }
        self.resolved_conflicts as f64 / self.total_conflicts as f64
    }
}

impl fmt::Display for MergeSessionSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SessionSummary(files={}/{}, skipped={}, conflicts={}/{})",
            self.resolved_files, self.total_files,
            self.skipped_files,
            self.resolved_conflicts, self.total_conflicts
        )
    }
}

// ---------------------------------------------------------------------------
// MergeConflict — word-level diff helpers
// ---------------------------------------------------------------------------

impl MergeConflict {
    /// Returns the number of words that differ between the current and incoming
    /// text. Useful for sizing up a conflict at a glance.
    pub fn word_diff_count(&self) -> usize {
        let cur_words: Vec<&str> = self.current_text.split_whitespace().collect();
        let inc_words: Vec<&str> = self.incoming_text.split_whitespace().collect();
        let max_len = cur_words.len().max(inc_words.len());
        let mut diffs = 0;
        for i in 0..max_len {
            if cur_words.get(i) != inc_words.get(i) {
                diffs += 1;
            }
        }
        diffs
    }

    /// True when the conflict only involves whitespace changes.
    pub fn is_whitespace_only(&self) -> bool {
        let cur_stripped: String =
            self.current_text.chars().filter(|c| !c.is_whitespace()).collect();
        let inc_stripped: String =
            self.incoming_text.chars().filter(|c| !c.is_whitespace()).collect();
        cur_stripped == inc_stripped && self.current_text != self.incoming_text
    }

    /// Reset a previously resolved conflict back to unresolved.
    pub fn unresolve(&mut self) {
        self.resolved = false;
        self.resolution = None;
    }
}

// ---------------------------------------------------------------------------
// MergeEditorWidget — batch & search operations
// ---------------------------------------------------------------------------

impl MergeEditorWidget {
    /// Resolve all remaining conflicts with the given resolution strategy.
    pub fn resolve_all(&mut self, resolution: MergeResolution) {
        for i in 0..self.conflicts.len() {
            if !self.conflicts[i].resolved {
                self.resolve_conflict(i, resolution.clone());
            }
        }
    }

    /// Reset every conflict back to unresolved.
    pub fn unresolve_all(&mut self) {
        for c in &mut self.conflicts {
            c.unresolve();
        }
    }

    /// Find the first conflict whose current or incoming text contains `needle`.
    pub fn find_conflict_containing(&self, needle: &str) -> Option<usize> {
        self.conflicts.iter().position(|c| {
            c.current_text.contains(needle) || c.incoming_text.contains(needle)
        })
    }

    /// Collect indices of conflicts that are whitespace-only changes.
    pub fn whitespace_only_indices(&self) -> Vec<usize> {
        self.conflicts
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_whitespace_only())
            .map(|(i, _)| i)
            .collect()
    }

    /// Auto-resolve whitespace-only conflicts by accepting the incoming side.
    pub fn auto_resolve_whitespace(&mut self) -> usize {
        let mut count = 0;
        for c in &mut self.conflicts {
            if !c.resolved && c.is_whitespace_only() {
                c.resolution = Some(c.incoming_text.clone());
                c.resolved = true;
                count += 1;
            }
        }
        count
    }

    /// Returns the largest conflict measured by `word_diff_count`.
    pub fn largest_conflict_index(&self) -> Option<usize> {
        self.conflicts
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| c.word_diff_count())
            .map(|(i, _)| i)
    }
}

// ---------------------------------------------------------------------------
// MergeSession — bulk operations
// ---------------------------------------------------------------------------

impl MergeSession {
    /// Return paths of all files matching a given status.
    pub fn files_with_status(&self, status: MergeFileStatus) -> Vec<&str> {
        self.files
            .iter()
            .filter(|f| f.status == status)
            .map(|f| f.path.as_str())
            .collect()
    }

    /// Find a file entry by path, returning its index.
    pub fn find_file(&self, path: &str) -> Option<usize> {
        self.files.iter().position(|f| f.path == path)
    }

    /// Jump to a file by path, returning `true` if found.
    pub fn jump_to_file(&mut self, path: &str) -> bool {
        if let Some(idx) = self.find_file(path) {
            self.current_file = idx;
            true
        } else {
            false
        }
    }

    /// Total number of unresolved conflicts across all files.
    pub fn total_unresolved_conflicts(&self) -> usize {
        self.files.iter().map(|f| f.editor.unresolved_count()).sum()
    }
}


// ---------------------------------------------------------------------------
// MergeConflictDetector
// ---------------------------------------------------------------------------

pub struct MergeConflictDetector;

impl MergeConflictDetector {
    /// Detect conflict markers in text and return their line ranges.
    pub fn detect_markers(text: &str) -> Vec<(usize, usize)> {
        let lines: Vec<&str> = text.lines().collect();
        let mut conflicts = Vec::new();
        let mut start = None;
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("<<<<<<<") { start = Some(i); }
            if line.starts_with(">>>>>>>") {
                if let Some(s) = start {
                    conflicts.push((s, i));
                    start = None;
                }
            }
        }
        conflicts
    }

    /// Count the number of conflict regions.
    pub fn conflict_count(text: &str) -> usize {
        Self::detect_markers(text).len()
    }

    /// Check if text has any unresolved conflicts.
    pub fn has_conflicts(text: &str) -> bool {
        Self::conflict_count(text) > 0
    }

    /// Extract the content between conflict markers at a given index.
    pub fn extract_conflict_text(text: &str, index: usize) -> Option<String> {
        let markers = Self::detect_markers(text);
        markers.get(index).map(|&(start, end)| {
            let lines: Vec<&str> = text.lines().collect();
            lines[start..=end].join("\n")
        })
    }
}

// ---------------------------------------------------------------------------
// MergeAutoResolver
// ---------------------------------------------------------------------------

pub struct MergeAutoResolver;

impl MergeAutoResolver {
    /// Attempt to auto-resolve a conflict where one side is empty.
    pub fn try_auto_resolve(conflict: &MergeConflict) -> Option<MergeResolution> {
        if conflict.incoming_text.trim().is_empty() {
            return Some(MergeResolution::AcceptCurrent);
        }
        if conflict.current_text.trim().is_empty() {
            return Some(MergeResolution::AcceptIncoming);
        }
        if conflict.current_text == conflict.incoming_text {
            return Some(MergeResolution::AcceptCurrent);
        }
        None
    }

    /// Auto-resolve all trivial conflicts in a widget, returning a vec of resolutions.
    pub fn auto_resolve_all(widget: &MergeEditorWidget) -> Vec<Option<MergeResolution>> {
        let mut resolutions: Vec<Option<MergeResolution>> = vec![None; widget.conflicts.len()];
        for i in 0..widget.conflicts.len() {
            if let Some(res) = Self::try_auto_resolve(&widget.conflicts[i]) {
                resolutions[i] = Some(res);
            }
        }
        resolutions
    }
}

// ---------------------------------------------------------------------------
// MergeBase3WayViewer
// ---------------------------------------------------------------------------

pub struct MergeBase3WayViewer {
    pub base: String,
    pub current: String,
    pub incoming: String,
}

impl MergeBase3WayViewer {
    pub fn new(base: impl Into<String>, current: impl Into<String>, incoming: impl Into<String>) -> Self {
        Self { base: base.into(), current: current.into(), incoming: incoming.into() }
    }

    pub fn base_lines(&self) -> Vec<&str> { self.base.lines().collect() }
    pub fn current_lines(&self) -> Vec<&str> { self.current.lines().collect() }
    pub fn incoming_lines(&self) -> Vec<&str> { self.incoming.lines().collect() }

    pub fn has_base_changes(&self) -> bool { self.base != self.current }
    pub fn has_incoming_changes(&self) -> bool { self.base != self.incoming }
    pub fn is_trivial(&self) -> bool { !self.has_base_changes() || !self.has_incoming_changes() }
}

impl fmt::Display for MergeBase3WayViewer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "3Way(base={} lines, current={} lines, incoming={} lines)",
            self.base_lines().len(), self.current_lines().len(), self.incoming_lines().len())
    }
}

// ---------------------------------------------------------------------------
// MergeResultPreview
// ---------------------------------------------------------------------------

pub struct MergeResultPreview {
    preview_lines: Vec<String>,
    has_unresolved: bool,
}

impl MergeResultPreview {
    pub fn from_widget(widget: &MergeEditorWidget) -> Self {
        let lines = widget.get_merged_result();
        let has_unresolved = widget.unresolved_count() > 0;
        Self { preview_lines: lines, has_unresolved }
    }

    pub fn lines(&self) -> &[String] { &self.preview_lines }
    pub fn line_count(&self) -> usize { self.preview_lines.len() }
    pub fn has_unresolved(&self) -> bool { self.has_unresolved }

    pub fn as_text(&self) -> String { self.preview_lines.join("\n") }
}

impl fmt::Display for MergeResultPreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Preview({} lines, unresolved={})", self.line_count(), self.has_unresolved)
    }
}

// ---------------------------------------------------------------------------
// MergeConflictCounter – counts and categorizes conflicts in a merge
// ---------------------------------------------------------------------------

/// Category of a merge conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictCategory {
    /// Both sides modified the same lines differently.
    ContentConflict,
    /// One side deleted lines the other modified.
    DeleteModify,
    /// Both sides added content at the same location.
    AddAdd,
    /// Whitespace-only conflict.
    WhitespaceOnly,
}

/// Summary statistics for conflicts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConflictCountSummary {
    pub content: usize,
    pub delete_modify: usize,
    pub add_add: usize,
    pub whitespace_only: usize,
}

impl ConflictCountSummary {
    pub fn total(&self) -> usize {
        self.content + self.delete_modify + self.add_add + self.whitespace_only
    }

    pub fn has_real_conflicts(&self) -> bool {
        self.content > 0 || self.delete_modify > 0 || self.add_add > 0
    }

    pub fn increment(&mut self, cat: ConflictCategory) {
        match cat {
            ConflictCategory::ContentConflict => self.content += 1,
            ConflictCategory::DeleteModify => self.delete_modify += 1,
            ConflictCategory::AddAdd => self.add_add += 1,
            ConflictCategory::WhitespaceOnly => self.whitespace_only += 1,
        }
    }
}

impl fmt::Display for ConflictCountSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "conflicts: {} total (content={}, delete/modify={}, add/add={}, whitespace={})",
            self.total(),
            self.content,
            self.delete_modify,
            self.add_add,
            self.whitespace_only,
        )
    }
}

/// Counts and categorizes conflicts within merge content.
#[derive(Debug)]
pub struct MergeConflictCounter;

impl MergeConflictCounter {
    /// Categorize a conflict based on the content of both sides.
    pub fn categorize(left: &str, right: &str) -> ConflictCategory {
        let left_trimmed = left.trim();
        let right_trimmed = right.trim();

        if left_trimmed == right_trimmed && left != right {
            return ConflictCategory::WhitespaceOnly;
        }
        if left_trimmed.is_empty() {
            return ConflictCategory::DeleteModify;
        }
        if right_trimmed.is_empty() {
            return ConflictCategory::DeleteModify;
        }
        // Both non-empty but different
        let left_lines: Vec<_> = left.lines().collect();
        let right_lines: Vec<_> = right.lines().collect();
        if left_lines.is_empty() && right_lines.is_empty() {
            ConflictCategory::AddAdd
        } else {
            ConflictCategory::ContentConflict
        }
    }

    /// Count conflicts in a list of (left, right) conflict pairs.
    pub fn count(pairs: &[(&str, &str)]) -> ConflictCountSummary {
        let mut summary = ConflictCountSummary::default();
        for &(left, right) in pairs {
            let cat = Self::categorize(left, right);
            summary.increment(cat);
        }
        summary
    }

    /// Percentage of conflicts that are trivial (whitespace-only).
    pub fn trivial_percentage(summary: &ConflictCountSummary) -> f64 {
        if summary.total() == 0 {
            return 0.0;
        }
        (summary.whitespace_only as f64 / summary.total() as f64) * 100.0
    }

    /// Estimate complexity: each content conflict scores 3, delete/modify scores 2,
    /// add/add scores 1, whitespace scores 0.
    pub fn complexity_score(summary: &ConflictCountSummary) -> usize {
        summary.content * 3 + summary.delete_modify * 2 + summary.add_add
    }
}

// ---------------------------------------------------------------------------
// MergeResultValidator – validates merge results for completeness
// ---------------------------------------------------------------------------

/// An issue found during merge result validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    /// Conflict markers still present in the output.
    UnresolvedMarker { line: usize },
    /// An empty line range that might indicate accidentally deleted content.
    SuspiciousEmptyRange { start_line: usize, end_line: usize },
    /// Duplicate consecutive lines that may be a merge artifact.
    DuplicateLines { line: usize, text: String },
    /// Trailing whitespace introduced by merge.
    TrailingWhitespace { line: usize },
}

/// Validates that a merge result is complete and free of artifacts.
#[derive(Debug)]
pub struct MergeResultValidator {
    check_markers: bool,
    check_duplicates: bool,
    check_trailing_ws: bool,
    max_allowed_empty_lines: usize,
}

impl MergeResultValidator {
    pub fn new() -> Self {
        Self {
            check_markers: true,
            check_duplicates: true,
            check_trailing_ws: false,
            max_allowed_empty_lines: 3,
        }
    }

    pub fn with_trailing_ws_check(mut self, check: bool) -> Self {
        self.check_trailing_ws = check;
        self
    }

    pub fn with_max_empty_lines(mut self, max: usize) -> Self {
        self.max_allowed_empty_lines = max;
        self
    }

    /// Validate merge result text, returning any issues found.
    pub fn validate(&self, text: &str) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        let mut empty_run_start: Option<usize> = None;
        let mut empty_run_len: usize = 0;

        for (i, line) in lines.iter().enumerate() {
            if self.check_markers {
                let trimmed = line.trim();
                if trimmed.starts_with("<<<<<<<")
                    || trimmed.starts_with(">>>>>>>")
                    || trimmed == "======="
                {
                    issues.push(ValidationIssue::UnresolvedMarker { line: i + 1 });
                }
            }

            if self.check_duplicates && i > 0 && *line == lines[i - 1] && !line.trim().is_empty() {
                issues.push(ValidationIssue::DuplicateLines {
                    line: i + 1,
                    text: line.to_string(),
                });
            }

            if self.check_trailing_ws && *line != line.trim_end() {
                issues.push(ValidationIssue::TrailingWhitespace { line: i + 1 });
            }

            if line.trim().is_empty() {
                if empty_run_start.is_none() {
                    empty_run_start = Some(i + 1);
                }
                empty_run_len += 1;
            } else {
                if empty_run_len > self.max_allowed_empty_lines {
                    if let Some(start) = empty_run_start {
                        issues.push(ValidationIssue::SuspiciousEmptyRange {
                            start_line: start,
                            end_line: start + empty_run_len - 1,
                        });
                    }
                }
                empty_run_start = None;
                empty_run_len = 0;
            }
        }

        // Check trailing empty run
        if empty_run_len > self.max_allowed_empty_lines {
            if let Some(start) = empty_run_start {
                issues.push(ValidationIssue::SuspiciousEmptyRange {
                    start_line: start,
                    end_line: start + empty_run_len - 1,
                });
            }
        }

        issues
    }

    /// Quick check: are there any unresolved conflict markers?
    pub fn has_unresolved_markers(&self, text: &str) -> bool {
        text.lines().any(|line| {
            let t = line.trim();
            t.starts_with("<<<<<<<") || t.starts_with(">>>>>>>") || t == "======="
        })
    }

    /// Count total issues by category.
    pub fn issue_counts(issues: &[ValidationIssue]) -> (usize, usize, usize, usize) {
        let mut markers = 0;
        let mut empty = 0;
        let mut dupes = 0;
        let mut ws = 0;
        for issue in issues {
            match issue {
                ValidationIssue::UnresolvedMarker { .. } => markers += 1,
                ValidationIssue::SuspiciousEmptyRange { .. } => empty += 1,
                ValidationIssue::DuplicateLines { .. } => dupes += 1,
                ValidationIssue::TrailingWhitespace { .. } => ws += 1,
            }
        }
        (markers, empty, dupes, ws)
    }
}



// ─── MrgBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for merge resolutions.
#[derive(Debug, Clone)]
pub struct MrgBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> MrgBufRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for MrgBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MrgBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── MrgFmt Formatter ───────────────────────────────────────

/// Formatting options for merge editor output.
#[derive(Debug, Clone)]
pub struct MrgFmtFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for MrgFmtFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl MrgFmtFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for merge editor data.
pub struct MrgFmtFmt {
    options: MrgFmtFmtOpts,
}

impl MrgFmtFmt {
    pub fn new(options: MrgFmtFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: MrgFmtFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}


/// Configuration manager for merge_editor functionality.
pub struct MergeEditorConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl MergeEditorConfig {
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

    pub fn merge(&mut self, other: &MergeEditorConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for merge_editor operations.
pub struct MergeEditorRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl MergeEditorRateTracker {
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

/// Validation result collector for merge_editor.
pub struct MergeEditorValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl MergeEditorValidator {
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

    pub fn merge(&mut self, other: &MergeEditorValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Three-way merge editor — extended utilities (xi)
// ---------------------------------------------------------------------------

/// Metric accumulator for merge_ed operations.
#[derive(Debug, Clone)]
pub struct XiMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XiMetrics {
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

/// Sliding-window rate counter for merge_ed.
#[derive(Debug, Clone)]
pub struct XiRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XiRateWindow {
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

/// A small LRU-style cache for merge_ed lookups.
#[derive(Debug, Clone)]
pub struct XiLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XiLruCache {
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
// xb_ utilities – batch 26
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer26 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer26 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_26(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_26<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_26<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_26(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_26(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 123
// ---------------------------------------------------------------------------

/// Generic object pool `Xc123Pool<T>`.
pub struct Xc123Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc123Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc123PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc123Pool<T> {
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
    pub fn stats(&self) -> Xc123PoolStats {
        Xc123PoolStats {
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

impl<T> Default for Xc123Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc123Scheduler`.
pub struct Xc123Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc123Scheduler {
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

impl Default for Xc123Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_123 hash for the given byte slice.
pub fn xc_123_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_123 convention.
pub fn xc_123_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe38 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe38Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe38PipelineError {
    pub stage: Xe38Stage,
    pub message: String,
}

impl std::fmt::Display for Xe38PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe38Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe38Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError>>>,
    stage_names: Vec<Xe38Stage>,
}

impl Xe38Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe38Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe38Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe38Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe38Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe38Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe38CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe38CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe38Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe38CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe38CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe38Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe38CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_38_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe38CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_38_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe38CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_38_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError> {
    Ok(data)
}

pub fn xe_38_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_38_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_38_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_38_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe38PipelineError> {
    Err(Xe38PipelineError {
        stage: Xe38Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_5: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg5Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg5Graph {
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

impl Default for Xg5Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_5: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg5Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg5Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg5Heap<T>) {
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

impl<T: Ord> Default for Xg5Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 122).
pub struct Xh122SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh122SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 164 as u64,
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

/// A compact bit set supporting boolean operations (variant 122).
pub struct Xh122BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh122BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 122).
pub struct Xi122Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi122Deque<T> {
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
pub struct Xi122Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi122Interval {
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

/// A simple interval tree (variant 122).
pub struct Xi122IntervalTree {
    xi_intervals: Vec<Xi122Interval>,
}

impl Xi122IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi122Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi122Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi122Interval) -> Vec<&Xi122Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi122Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi122Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi122Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi122Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi122Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi122Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 122) ---

/// Disjoint set / union-find for crate 122.
pub struct Xj122UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj122UnionFind {
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

const XJ122_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 122.
pub struct Xj122BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj122BTreeNode<K, V>>>,
    len: usize,
}

struct Xj122BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj122BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj122BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ122_BTREE_ORDER - 1
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
        let mid = XJ122_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj122BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj122BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj122BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj122BTreeNode::xj_new_leaf();
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


// --- xk_122 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk122SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk122SegmentTree {
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
pub struct Xk122DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk122DisjointIntervals {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_conflict() {
        let text = "before\n<<<<<<< HEAD\ncurrent line\n=======\nincoming line\n>>>>>>> branch\nafter";
        let conflicts = parse_conflict_markers(text);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].current_text, "current line");
        assert_eq!(conflicts[0].incoming_text, "incoming line");
        assert!(!conflicts[0].resolved);
    }

    #[test]
    fn parse_multiple_conflicts() {
        let text = "\
<<<<<<< HEAD
a
=======
b
>>>>>>> branch
middle
<<<<<<< HEAD
c
=======
d
>>>>>>> branch";
        let conflicts = parse_conflict_markers(text);
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].current_text, "a");
        assert_eq!(conflicts[1].incoming_text, "d");
    }

    #[test]
    fn resolve_accept_current() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(MergeConflict {
            base_start: 0,
            base_end: 4,
            current_text: "ours".into(),
            incoming_text: "theirs".into(),
            base_text: "base".into(),
            resolved: false,
            resolution: None,
        });
        assert!(!w.all_resolved());
        w.resolve_conflict(0, MergeResolution::AcceptCurrent);
        assert!(w.all_resolved());
        assert_eq!(w.get_merged_result(), vec!["ours"]);
    }

    #[test]
    fn resolve_accept_both() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(MergeConflict {
            base_start: 0,
            base_end: 4,
            current_text: "ours".into(),
            incoming_text: "theirs".into(),
            base_text: String::new(),
            resolved: false,
            resolution: None,
        });
        w.resolve_conflict(0, MergeResolution::AcceptBoth);
        assert_eq!(w.get_merged_result(), vec!["ours\ntheirs"]);
    }

    #[test]
    fn navigation() {
        let mut w = MergeEditorWidget::new();
        for i in 0..3 {
            w.add_conflict(MergeConflict {
                base_start: i,
                base_end: i + 1,
                current_text: String::new(),
                incoming_text: String::new(),
                base_text: String::new(),
                resolved: false,
                resolution: None,
            });
        }
        assert_eq!(w.current_conflict, 0);
        w.next_conflict();
        assert_eq!(w.current_conflict, 1);
        w.next_conflict();
        assert_eq!(w.current_conflict, 2);
        w.next_conflict();
        assert_eq!(w.current_conflict, 2); // stays at end
        w.prev_conflict();
        assert_eq!(w.current_conflict, 1);
        w.prev_conflict();
        assert_eq!(w.current_conflict, 0);
        w.prev_conflict();
        assert_eq!(w.current_conflict, 0); // stays at start
    }

    #[test]
    fn parse_diff3_base_markers() {
        let text = "<<<<<<< HEAD\nours\n||||||| merged common\nbase\n=======\ntheirs\n>>>>>>> branch";
        let conflicts = parse_conflict_markers(text);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].current_text, "ours");
        assert_eq!(conflicts[0].base_text, "base");
        assert_eq!(conflicts[0].incoming_text, "theirs");
    }

    #[test]
    fn builder_creates_valid_conflict() {
        let c = MergeConflictBuilder::new()
            .region(0, 5)
            .current_text("ours")
            .incoming_text("theirs")
            .base_text("base")
            .build()
            .unwrap();
        assert_eq!(c.line_span(), 5);
        assert!(!c.resolved);
    }

    #[test]
    fn builder_rejects_invalid_region() {
        let err = MergeConflictBuilder::new()
            .region(5, 3)
            .build()
            .unwrap_err();
        assert_eq!(
            err,
            MergeError::InvalidRegion { start: 5, end: 3 }
        );
    }

    #[test]
    fn trivial_conflict_detection() {
        let c = MergeConflictBuilder::new()
            .region(0, 2)
            .current_text("same")
            .incoming_text("same")
            .build()
            .unwrap();
        assert!(c.is_trivial());
    }

    #[test]
    fn auto_resolve_trivial_conflicts() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 1)
                .current_text("same")
                .incoming_text("same")
                .build()
                .unwrap(),
        );
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(1, 3)
                .current_text("a")
                .incoming_text("b")
                .build()
                .unwrap(),
        );
        let auto = w.auto_resolve_trivial();
        assert_eq!(auto, 1);
        assert_eq!(w.resolved_count(), 1);
        assert_eq!(w.unresolved_count(), 1);
    }

    #[test]
    fn try_resolve_out_of_range() {
        let mut w = MergeEditorWidget::new();
        let err = w
            .try_resolve_conflict(0, MergeResolution::AcceptCurrent)
            .unwrap_err();
        assert_eq!(
            err,
            MergeError::ConflictOutOfRange { index: 0, total: 0 }
        );
    }

    #[test]
    fn try_resolve_empty_custom() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 2)
                .current_text("a")
                .incoming_text("b")
                .build()
                .unwrap(),
        );
        let err = w
            .try_resolve_conflict(0, MergeResolution::Custom(String::new()))
            .unwrap_err();
        assert_eq!(err, MergeError::EmptyCustomResolution);
    }

    #[test]
    fn try_get_merged_result_fails_when_unresolved() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 2)
                .current_text("a")
                .incoming_text("b")
                .build()
                .unwrap(),
        );
        let err = w.try_get_merged_result().unwrap_err();
        assert_eq!(err, MergeError::UnresolvedConflicts { remaining: 1 });
    }

    #[test]
    fn toggle_mode() {
        let mut w = MergeEditorWidget::new();
        assert_eq!(w.mode, MergeEditorMode::Inline);
        w.toggle_mode();
        assert_eq!(w.mode, MergeEditorMode::SideBySide);
        w.toggle_mode();
        assert_eq!(w.mode, MergeEditorMode::Inline);
    }

    #[test]
    fn text_for_side() {
        let c = MergeConflictBuilder::new()
            .region(0, 1)
            .current_text("cur")
            .incoming_text("inc")
            .base_text("bas")
            .build()
            .unwrap();
        assert_eq!(c.text_for_side(ConflictSide::Current), "cur");
        assert_eq!(c.text_for_side(ConflictSide::Incoming), "inc");
        assert_eq!(c.text_for_side(ConflictSide::Base), "bas");
    }

    #[test]
    fn display_impls() {
        let c = MergeConflictBuilder::new()
            .region(0, 5)
            .current_text("a")
            .incoming_text("b")
            .build()
            .unwrap();
        assert!(c.to_string().contains("unresolved"));

        let w = MergeEditorWidget::new();
        assert!(w.to_string().contains("0/0 resolved"));

        assert_eq!(ConflictSide::Current.to_string(), "Current");
        assert_eq!(MergeResolution::AcceptBoth.to_string(), "Accept Both");
        assert_eq!(MergeEditorMode::Inline.to_string(), "Inline");
    }

    #[test]
    fn parse_no_conflicts() {
        let text = "just some\nplain text\nno markers";
        let conflicts = parse_conflict_markers(text);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn conflict_stats_computation() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 3)
                .current_text("same")
                .incoming_text("same")
                .build()
                .unwrap(),
        );
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(3, 7)
                .current_text("a")
                .incoming_text("b")
                .build()
                .unwrap(),
        );
        let stats = w.conflict_stats();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.unresolved, 2);
        assert_eq!(stats.trivial, 1);
        assert_eq!(stats.total_lines, 7);
    }

    #[test]
    fn validate_result_all_resolved() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 2)
                .current_text("a")
                .incoming_text("b")
                .build()
                .unwrap(),
        );
        w.resolve_conflict(0, MergeResolution::AcceptCurrent);
        assert!(w.validate_result().is_ok());
    }

    #[test]
    fn validate_result_unresolved() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 2)
                .current_text("a")
                .incoming_text("b")
                .build()
                .unwrap(),
        );
        assert!(w.validate_result().is_err());
    }

    #[test]
    fn auto_resolve_non_overlapping() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(MergeConflict {
            base_start: 0,
            base_end: 2,
            current_text: "base".into(),
            incoming_text: "changed".into(),
            base_text: "base".into(),
            resolved: false,
            resolution: None,
        });
        w.add_conflict(MergeConflict {
            base_start: 2,
            base_end: 4,
            current_text: "modified".into(),
            incoming_text: "original".into(),
            base_text: "original".into(),
            resolved: false,
            resolution: None,
        });
        let count = w.auto_resolve_non_overlapping();
        assert_eq!(count, 2);
        assert!(w.all_resolved());
        assert_eq!(w.conflicts[0].resolution.as_deref(), Some("changed"));
        assert_eq!(w.conflicts[1].resolution.as_deref(), Some("modified"));
    }

    #[test]
    fn generate_preview_mixed() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 2)
                .current_text("ours")
                .incoming_text("theirs")
                .build()
                .unwrap(),
        );
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(2, 4)
                .current_text("x")
                .incoming_text("y")
                .build()
                .unwrap(),
        );
        w.resolve_conflict(0, MergeResolution::AcceptCurrent);
        let preview = w.generate_preview();
        assert_eq!(preview[0], "ours");
        assert!(preview.iter().any(|l| l.contains("<<<<<<< Current")));
    }

    #[test]
    fn unresolved_indices() {
        let mut w = MergeEditorWidget::new();
        for i in 0..4 {
            w.add_conflict(
                MergeConflictBuilder::new()
                    .region(i, i + 1)
                    .current_text("a")
                    .incoming_text("b")
                    .build()
                    .unwrap(),
            );
        }
        w.resolve_conflict(1, MergeResolution::AcceptCurrent);
        w.resolve_conflict(3, MergeResolution::AcceptIncoming);
        assert_eq!(w.unresolved_indices(), vec![0, 2]);
    }

    #[test]
    fn conflict_stats_display() {
        let stats = ConflictStats {
            total: 5,
            resolved: 3,
            unresolved: 2,
            trivial: 1,
            total_lines: 20,
        };
        let s = format!("{stats}");
        assert!(s.contains("total=5"));
        assert!(s.contains("resolved=3"));
    }

    // ---- ThreeWayMerge tests ----

    #[test]
    fn three_way_merge_no_conflicts_when_sides_agree() {
        let m = ThreeWayMerge::new(
            vec!["a".into()],
            vec!["b".into()],
            vec!["b".into()],
        );
        assert!(!m.has_conflicts());
    }

    #[test]
    fn three_way_merge_has_conflicts_both_differ() {
        let m = ThreeWayMerge::new(
            vec!["a".into()],
            vec!["b".into()],
            vec!["c".into()],
        );
        assert!(m.has_conflicts());
    }

    #[test]
    fn three_way_merge_line_counts() {
        let m = ThreeWayMerge::new(
            vec!["1".into(), "2".into()],
            vec!["a".into()],
            vec!["x".into(), "y".into(), "z".into()],
        );
        assert_eq!(m.base_line_count(), 2);
        assert_eq!(m.ours_line_count(), 1);
        assert_eq!(m.theirs_line_count(), 3);
    }

    // ---- conflict_regions tests ----

    #[test]
    fn conflict_regions_no_conflicts() {
        let base = vec!["a".into(), "b".into()];
        let ours = vec!["a".into(), "b".into()];
        let theirs = vec!["a".into(), "b".into()];
        let regions = conflict_regions(&base, &ours, &theirs);
        assert!(regions.is_empty());
    }

    #[test]
    fn conflict_regions_single_conflict() {
        let base = vec!["a".into(), "b".into(), "c".into()];
        let ours = vec!["a".into(), "X".into(), "c".into()];
        let theirs = vec!["a".into(), "Y".into(), "c".into()];
        let regions = conflict_regions(&base, &ours, &theirs);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].base_start, 1);
        assert_eq!(regions[0].base_end, 2);
        assert_eq!(regions[0].ours_lines, vec!["X".to_string()]);
        assert_eq!(regions[0].theirs_lines, vec!["Y".to_string()]);
    }

    #[test]
    fn conflict_regions_only_one_side_differs() {
        let base = vec!["a".into(), "b".into()];
        let ours = vec!["a".into(), "X".into()];
        let theirs = vec!["a".into(), "b".into()];
        let regions = conflict_regions(&base, &ours, &theirs);
        assert!(regions.is_empty());
    }

    // ---- auto_resolve tests ----

    #[test]
    fn auto_resolve_identical() {
        let base = vec!["a".into(), "b".into()];
        let result = auto_resolve(&base, &base, &base);
        assert_eq!(result.merged_lines, vec!["a".to_string(), "b".to_string()]);
        assert!(!result.had_conflicts);
        assert_eq!(result.conflict_count, 0);
    }

    #[test]
    fn auto_resolve_only_ours_differs() {
        let base = vec!["a".into(), "b".into()];
        let ours = vec!["a".into(), "X".into()];
        let result = auto_resolve(&base, &ours, &base);
        assert_eq!(result.merged_lines, vec!["a".to_string(), "X".to_string()]);
        assert!(!result.had_conflicts);
    }

    #[test]
    fn auto_resolve_only_theirs_differs() {
        let base = vec!["a".into(), "b".into()];
        let theirs = vec!["a".into(), "Y".into()];
        let result = auto_resolve(&base, &base, &theirs);
        assert_eq!(result.merged_lines, vec!["a".to_string(), "Y".to_string()]);
        assert!(!result.had_conflicts);
    }

    #[test]
    fn auto_resolve_both_agree_on_change() {
        let base = vec!["a".into()];
        let changed = vec!["Z".into()];
        let result = auto_resolve(&base, &changed, &changed);
        assert_eq!(result.merged_lines, vec!["Z".to_string()]);
        assert!(!result.had_conflicts);
    }

    #[test]
    fn auto_resolve_true_conflict_produces_markers() {
        let base = vec!["a".into(), "b".into(), "c".into()];
        let ours = vec!["a".into(), "X".into(), "c".into()];
        let theirs = vec!["a".into(), "Y".into(), "c".into()];
        let result = auto_resolve(&base, &ours, &theirs);
        assert!(result.had_conflicts);
        assert_eq!(result.conflict_count, 1);
        assert!(result.merged_lines.contains(&"<<<<<<< ours".to_string()));
        assert!(result.merged_lines.contains(&"=======".to_string()));
        assert!(result.merged_lines.contains(&">>>>>>> theirs".to_string()));
        assert!(result.merged_lines.contains(&"X".to_string()));
        assert!(result.merged_lines.contains(&"Y".to_string()));
    }

    #[test]
    fn auto_resolve_multiple_conflicts() {
        let base = vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()];
        let ours = vec!["X".into(), "b".into(), "c".into(), "W".into(), "e".into()];
        let theirs = vec!["Y".into(), "b".into(), "c".into(), "V".into(), "e".into()];
        let result = auto_resolve(&base, &ours, &theirs);
        assert!(result.had_conflicts);
        assert_eq!(result.conflict_count, 2);
    }

    // ---- ConflictsBySource tests ----

    #[test]
    fn conflicts_by_source_breakdown() {
        let mut widget = MergeEditorWidget::new();
        // current_only: current differs, incoming == base
        widget.add_conflict(MergeConflictBuilder::new().region(0, 2).current_text("X").incoming_text("base").base_text("base").build().unwrap());
        // incoming_only: incoming differs, current == base
        widget.add_conflict(MergeConflictBuilder::new().region(2, 4).current_text("base").incoming_text("Y").base_text("base").build().unwrap());
        // both changed
        widget.add_conflict(MergeConflictBuilder::new().region(4, 6).current_text("A").incoming_text("B").base_text("base").build().unwrap());
        // trivial: current == incoming
        widget.add_conflict(MergeConflictBuilder::new().region(6, 8).current_text("same").incoming_text("same").base_text("base").build().unwrap());

        let by_source = widget.conflicts_by_source();
        assert_eq!(by_source.current_only, 1);
        assert_eq!(by_source.incoming_only, 1);
        assert_eq!(by_source.both_changed, 1);
        assert_eq!(by_source.trivial, 1);
    }

    #[test]
    fn resolution_ratio_computation() {
        let mut widget = MergeEditorWidget::new();
        widget.add_conflict(MergeConflictBuilder::new().region(0, 2).current_text("a").incoming_text("b").base_text("c").build().unwrap());
        widget.add_conflict(MergeConflictBuilder::new().region(2, 4).current_text("d").incoming_text("e").base_text("f").build().unwrap());
        assert!((widget.resolution_ratio() - 0.0).abs() < f64::EPSILON);

        widget.resolve_conflict(0, MergeResolution::AcceptCurrent);
        assert!((widget.resolution_ratio() - 0.5).abs() < f64::EPSILON);

        widget.resolve_conflict(1, MergeResolution::AcceptIncoming);
        assert!((widget.resolution_ratio() - 1.0).abs() < f64::EPSILON);
    }

    // ---- MergeSession tests ----

    #[test]
    fn merge_session_file_tracking() {
        let mut session = MergeSession::new();
        session.add_file("file_a.rs");
        session.add_file("file_b.rs");
        session.add_file("file_c.rs");
        assert_eq!(session.file_count(), 3);
        assert_eq!(session.resolved_file_count(), 0);
        assert!(!session.is_complete());

        // Navigate
        assert!(session.next_file());
        assert_eq!(session.current_file, 1);
        assert!(session.prev_file());
        assert_eq!(session.current_file, 0);
    }

    #[test]
    fn merge_session_skip_and_progress() {
        let mut session = MergeSession::new();
        session.add_file("a.rs");
        session.add_file("b.rs");
        session.skip_current();
        assert!((session.progress() - 0.5).abs() < f64::EPSILON);
        session.files[1].status = MergeFileStatus::Resolved;
        assert!(session.is_complete());
        assert!((session.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn merge_result_record_serialization() {
        let mut session = MergeSession::new();
        session.add_file("test.rs");
        {
            let entry = session.current_entry_mut().unwrap();
            entry.editor.add_conflict(
                MergeConflictBuilder::new().region(0, 2).current_text("a").incoming_text("b").base_text("c").build().unwrap(),
            );
            entry.editor.resolve_conflict(0, MergeResolution::AcceptCurrent);
            entry.status = MergeFileStatus::InProgress;
            entry.try_finish();
        }
        let records = serialize_session_results(&session);
        assert_eq!(records.len(), 1);
        assert!(records[0].is_fully_resolved());
        assert_eq!(records[0].merged_lines, vec!["a".to_string()]);
    }

    #[test]
    fn merge_session_summary() {
        let mut session = MergeSession::new();
        session.add_file("x.rs");
        session.add_file("y.rs");
        {
            let entry = &mut session.files[0];
            entry.editor.add_conflict(
                MergeConflictBuilder::new().region(0, 2).current_text("a").incoming_text("b").base_text("c").build().unwrap(),
            );
            entry.editor.resolve_conflict(0, MergeResolution::AcceptCurrent);
            entry.status = MergeFileStatus::Resolved;
        }
        session.files[1].status = MergeFileStatus::Skipped;

        let summary = MergeSessionSummary::from_session(&session);
        assert_eq!(summary.total_files, 2);
        assert_eq!(summary.resolved_files, 1);
        assert_eq!(summary.skipped_files, 1);
        assert_eq!(summary.total_conflicts, 1);
        assert_eq!(summary.resolved_conflicts, 1);
        assert!((summary.conflict_resolution_ratio() - 1.0).abs() < f64::EPSILON);
    }

    // ---- New functionality tests ----

    #[test]
    fn word_diff_count_identical() {
        let c = MergeConflictBuilder::new()
            .region(0, 2)
            .current_text("hello world")
            .incoming_text("hello world")
            .build()
            .unwrap();
        assert_eq!(c.word_diff_count(), 0);
    }

    #[test]
    fn word_diff_count_different() {
        let c = MergeConflictBuilder::new()
            .region(0, 2)
            .current_text("the quick fox")
            .incoming_text("the slow bear")
            .build()
            .unwrap();
        assert_eq!(c.word_diff_count(), 2);
    }

    #[test]
    fn is_whitespace_only_true() {
        let c = MergeConflictBuilder::new()
            .region(0, 2)
            .current_text("hello  world")
            .incoming_text("hello world")
            .build()
            .unwrap();
        assert!(c.is_whitespace_only());
    }

    #[test]
    fn is_whitespace_only_false_when_identical() {
        let c = MergeConflictBuilder::new()
            .region(0, 2)
            .current_text("hello")
            .incoming_text("hello")
            .build()
            .unwrap();
        // identical texts are not "whitespace-only changes"
        assert!(!c.is_whitespace_only());
    }

    #[test]
    fn unresolve_resets_conflict() {
        let mut c = MergeConflictBuilder::new()
            .region(0, 2)
            .current_text("a")
            .incoming_text("b")
            .build()
            .unwrap();
        c.resolved = true;
        c.resolution = Some("a".into());
        c.unresolve();
        assert!(!c.resolved);
        assert!(c.resolution.is_none());
    }

    #[test]
    fn resolve_all_accepts_incoming() {
        let mut w = MergeEditorWidget::new();
        for i in 0..3 {
            w.add_conflict(
                MergeConflictBuilder::new()
                    .region(i, i + 1)
                    .current_text("a")
                    .incoming_text("b")
                    .build()
                    .unwrap(),
            );
        }
        w.resolve_all(MergeResolution::AcceptIncoming);
        assert!(w.all_resolved());
        assert_eq!(w.get_merged_result(), vec!["b", "b", "b"]);
    }

    #[test]
    fn unresolve_all_clears_resolutions() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 2)
                .current_text("a")
                .incoming_text("b")
                .build()
                .unwrap(),
        );
        w.resolve_conflict(0, MergeResolution::AcceptCurrent);
        assert!(w.all_resolved());
        w.unresolve_all();
        assert_eq!(w.unresolved_count(), 1);
        assert!(!w.all_resolved());
    }

    #[test]
    fn find_conflict_containing_text() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 1)
                .current_text("fn main()")
                .incoming_text("fn start()")
                .build()
                .unwrap(),
        );
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(1, 2)
                .current_text("let x = 1")
                .incoming_text("let y = 2")
                .build()
                .unwrap(),
        );
        assert_eq!(w.find_conflict_containing("main"), Some(0));
        assert_eq!(w.find_conflict_containing("let y"), Some(1));
        assert_eq!(w.find_conflict_containing("nonexistent"), None);
    }

    #[test]
    fn whitespace_only_indices_and_auto_resolve() {
        let mut w = MergeEditorWidget::new();
        // whitespace-only conflict
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 1)
                .current_text("a  b")
                .incoming_text("a b")
                .build()
                .unwrap(),
        );
        // real conflict
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(1, 2)
                .current_text("foo")
                .incoming_text("bar")
                .build()
                .unwrap(),
        );
        assert_eq!(w.whitespace_only_indices(), vec![0]);
        let resolved = w.auto_resolve_whitespace();
        assert_eq!(resolved, 1);
        assert!(w.conflicts[0].resolved);
        assert!(!w.conflicts[1].resolved);
    }

    #[test]
    fn largest_conflict_index_picks_biggest() {
        let mut w = MergeEditorWidget::new();
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 1)
                .current_text("a")
                .incoming_text("b")
                .build()
                .unwrap(),
        );
        w.add_conflict(
            MergeConflictBuilder::new()
                .region(1, 2)
                .current_text("the quick brown fox")
                .incoming_text("a slow red dog")
                .build()
                .unwrap(),
        );
        assert_eq!(w.largest_conflict_index(), Some(1));
    }

    #[test]
    fn session_files_with_status() {
        let mut session = MergeSession::new();
        session.add_file("a.rs");
        session.add_file("b.rs");
        session.add_file("c.rs");
        session.files[0].status = MergeFileStatus::Resolved;
        session.files[2].status = MergeFileStatus::Resolved;
        let resolved = session.files_with_status(MergeFileStatus::Resolved);
        assert_eq!(resolved, vec!["a.rs", "c.rs"]);
        let pending = session.files_with_status(MergeFileStatus::Pending);
        assert_eq!(pending, vec!["b.rs"]);
    }

    #[test]
    fn session_find_and_jump_to_file() {
        let mut session = MergeSession::new();
        session.add_file("alpha.rs");
        session.add_file("beta.rs");
        session.add_file("gamma.rs");
        assert_eq!(session.find_file("beta.rs"), Some(1));
        assert_eq!(session.find_file("missing.rs"), None);
        assert!(session.jump_to_file("gamma.rs"));
        assert_eq!(session.current_file, 2);
        assert!(!session.jump_to_file("nope.rs"));
        assert_eq!(session.current_file, 2); // unchanged
    }

    #[test]
    fn session_total_unresolved_conflicts() {
        let mut session = MergeSession::new();
        session.add_file("a.rs");
        session.add_file("b.rs");
        session.files[0].editor.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 2)
                .current_text("x")
                .incoming_text("y")
                .build()
                .unwrap(),
        );
        session.files[1].editor.add_conflict(
            MergeConflictBuilder::new()
                .region(0, 1)
                .current_text("p")
                .incoming_text("q")
                .build()
                .unwrap(),
        );
        session.files[1].editor.add_conflict(
            MergeConflictBuilder::new()
                .region(1, 3)
                .current_text("r")
                .incoming_text("s")
                .build()
                .unwrap(),
        );
        assert_eq!(session.total_unresolved_conflicts(), 3);
        session.files[0].editor.resolve_conflict(0, MergeResolution::AcceptCurrent);
        assert_eq!(session.total_unresolved_conflicts(), 2);
    }


    #[test]
    fn conflict_detector_basic() {
        let text = "line1\n<<<<<<< HEAD\ncurrent\n=======\nincoming\n>>>>>>> branch\nline2";
        assert_eq!(MergeConflictDetector::conflict_count(text), 1);
        assert!(MergeConflictDetector::has_conflicts(text));
    }

    #[test]
    fn conflict_detector_no_conflicts() {
        assert!(!MergeConflictDetector::has_conflicts("normal text"));
    }

    #[test]
    fn conflict_detector_multiple() {
        let text = "<<<<<<< HEAD\na\n=======\nb\n>>>>>>>\n<<<<<<< HEAD\nc\n=======\nd\n>>>>>>>";
        assert_eq!(MergeConflictDetector::conflict_count(text), 2);
    }

    #[test]
    fn conflict_detector_extract() {
        let text = "<<<<<<< HEAD\ncurrent\n=======\nincoming\n>>>>>>> branch";
        let extracted = MergeConflictDetector::extract_conflict_text(text, 0);
        assert!(extracted.is_some());
    }

    #[test]
    fn auto_resolver_empty_incoming() {
        let conflict = MergeConflictBuilder::new()
            .region(0, 5)
            .current_text("hello")
            .incoming_text("")
            .build().unwrap();
        assert_eq!(MergeAutoResolver::try_auto_resolve(&conflict), Some(MergeResolution::AcceptCurrent));
    }

    #[test]
    fn auto_resolver_empty_current() {
        let conflict = MergeConflictBuilder::new()
            .region(0, 5)
            .current_text("")
            .incoming_text("hello")
            .build().unwrap();
        assert_eq!(MergeAutoResolver::try_auto_resolve(&conflict), Some(MergeResolution::AcceptIncoming));
    }

    #[test]
    fn auto_resolver_identical() {
        let conflict = MergeConflictBuilder::new()
            .region(0, 5)
            .current_text("same")
            .incoming_text("same")
            .build().unwrap();
        assert_eq!(MergeAutoResolver::try_auto_resolve(&conflict), Some(MergeResolution::AcceptCurrent));
    }

    #[test]
    fn auto_resolver_no_auto() {
        let conflict = MergeConflictBuilder::new()
            .region(0, 5)
            .current_text("version a")
            .incoming_text("version b")
            .build().unwrap();
        assert_eq!(MergeAutoResolver::try_auto_resolve(&conflict), None);
    }

    #[test]
    fn three_way_viewer_basic() {
        let viewer = MergeBase3WayViewer::new("base", "current", "incoming");
        assert!(viewer.has_base_changes());
        assert!(viewer.has_incoming_changes());
        assert!(!viewer.is_trivial());
    }

    #[test]
    fn three_way_viewer_trivial() {
        let viewer = MergeBase3WayViewer::new("same", "same", "different");
        assert!(viewer.is_trivial());
    }

    #[test]
    fn result_preview() {
        let widget = MergeEditorWidget::new();
        let preview = MergeResultPreview::from_widget(&widget);
        assert!(!preview.has_unresolved());
    }

    #[test]
    fn three_way_display() {
        let v = MergeBase3WayViewer::new("a\nb", "c", "d");
        assert!(format!("{v}").contains("3Way"));
    }


    #[test]
    fn conflict_counter_content() {
        let cat = MergeConflictCounter::categorize("foo\nbar", "baz\nqux");
        assert_eq!(cat, ConflictCategory::ContentConflict);
    }

    #[test]
    fn conflict_counter_whitespace() {
        let cat = MergeConflictCounter::categorize("  hello  ", "hello");
        assert_eq!(cat, ConflictCategory::WhitespaceOnly);
    }

    #[test]
    fn conflict_counter_delete_modify() {
        let cat = MergeConflictCounter::categorize("", "something");
        assert_eq!(cat, ConflictCategory::DeleteModify);
    }

    #[test]
    fn conflict_summary_total() {
        let mut s = ConflictCountSummary::default();
        s.increment(ConflictCategory::ContentConflict);
        s.increment(ConflictCategory::ContentConflict);
        s.increment(ConflictCategory::WhitespaceOnly);
        assert_eq!(s.total(), 3);
        assert!(s.has_real_conflicts());
    }

    #[test]
    fn conflict_summary_display() {
        let s = ConflictCountSummary { content: 1, delete_modify: 2, add_add: 0, whitespace_only: 1 };
        let d = format!("{s}");
        assert!(d.contains("4 total"));
    }

    #[test]
    fn conflict_counter_count_pairs() {
        let pairs = vec![
            ("a", "b"),
            ("  x  ", "x"),
            ("", "y"),
        ];
        let summary = MergeConflictCounter::count(&pairs);
        assert_eq!(summary.content, 1);
        assert_eq!(summary.whitespace_only, 1);
        assert_eq!(summary.delete_modify, 1);
    }

    #[test]
    fn conflict_counter_trivial_percentage() {
        let s = ConflictCountSummary { content: 0, delete_modify: 0, add_add: 0, whitespace_only: 3 };
        assert!((MergeConflictCounter::trivial_percentage(&s) - 100.0).abs() < 0.01);
    }

    #[test]
    fn conflict_counter_complexity() {
        let s = ConflictCountSummary { content: 2, delete_modify: 1, add_add: 1, whitespace_only: 5 };
        assert_eq!(MergeConflictCounter::complexity_score(&s), 9); // 2*3 + 1*2 + 1*1
    }

    #[test]
    fn validator_clean_text() {
        let v = MergeResultValidator::new();
        let issues = v.validate("line1\nline2\nline3\n");
        assert!(issues.is_empty());
    }

    #[test]
    fn validator_unresolved_markers() {
        let v = MergeResultValidator::new();
        let text = "before\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> branch\nafter";
        let issues = v.validate(text);
        assert_eq!(MergeResultValidator::issue_counts(&issues).0, 3);
    }

    #[test]
    fn validator_has_unresolved() {
        let v = MergeResultValidator::new();
        assert!(v.has_unresolved_markers("<<<<<<< HEAD\n=======\n>>>>>>> b"));
        assert!(!v.has_unresolved_markers("clean text"));
    }

    #[test]
    fn validator_duplicate_lines() {
        let v = MergeResultValidator::new();
        let issues = v.validate("a\na\nb");
        assert_eq!(MergeResultValidator::issue_counts(&issues).2, 1);
    }

    #[test]
    fn validator_trailing_ws() {
        let v = MergeResultValidator::new().with_trailing_ws_check(true);
        let issues = v.validate("hello   \nworld");
        assert_eq!(MergeResultValidator::issue_counts(&issues).3, 1);
    }

    #[test]
    fn validator_suspicious_empty_range() {
        let v = MergeResultValidator::new().with_max_empty_lines(2);
        let text = "a\n\n\n\n\nb";
        let issues = v.validate(text);
        assert_eq!(MergeResultValidator::issue_counts(&issues).1, 1);
    }


    #[test]
    fn mrgbuf_ringbuf_push_get() {
        let mut rb = MrgBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn mrgbuf_ringbuf_overflow() {
        let mut rb = MrgBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn mrgbuf_ringbuf_clear() {
        let mut rb = MrgBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn mrgbuf_ringbuf_newest_oldest() {
        let mut rb = MrgBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn mrgbuf_ringbuf_to_vec() {
        let mut rb = MrgBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn mrgbuf_ringbuf_is_full() {
        let mut rb = MrgBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn mrgfmt_fmt_list() {
        let f = MrgFmtFmt::new(MrgFmtFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn mrgfmt_fmt_kv() {
        let f = MrgFmtFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn mrgfmt_fmt_section() {
        let f = MrgFmtFmt::new(MrgFmtFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn mrgfmt_fmt_truncate() {
        let f = MrgFmtFmt::new(MrgFmtFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn mrgfmt_fmt_opts_defaults() {
        let o = MrgFmtFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn merge_editor_config_new() {
        let cfg = MergeEditorConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn merge_editor_config_set_get() {
        let mut cfg = MergeEditorConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn merge_editor_config_remove() {
        let mut cfg = MergeEditorConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn merge_editor_config_keys_sorted() {
        let mut cfg = MergeEditorConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn merge_editor_config_bump_version() {
        let mut cfg = MergeEditorConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn merge_editor_config_clear() {
        let mut cfg = MergeEditorConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn merge_editor_config_merge() {
        let mut cfg1 = MergeEditorConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = MergeEditorConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn merge_editor_config_disable() {
        let mut cfg = MergeEditorConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn merge_editor_rate_tracker_empty() {
        let rt = MergeEditorRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn merge_editor_rate_tracker_record() {
        let mut rt = MergeEditorRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn merge_editor_rate_tracker_prune() {
        let mut rt = MergeEditorRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn merge_editor_validator_valid() {
        let v = MergeEditorValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn merge_editor_validator_errors() {
        let mut v = MergeEditorValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn merge_editor_validator_clear() {
        let mut v = MergeEditorValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn merge_editor_validator_merge() {
        let mut v1 = MergeEditorValidator::new();
        v1.add_error("e1");
        let mut v2 = MergeEditorValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn merge_editor_rate_tracker_clear() {
        let mut rt = MergeEditorRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn xi_metrics_empty() {
        let m = XiMetrics::new("merge_ed");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xi_metrics_record_and_mean() {
        let mut m = XiMetrics::new("merge_ed");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xi_metrics_min_max() {
        let mut m = XiMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xi_metrics_variance_and_std() {
        let mut m = XiMetrics::new("v");
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
    fn xi_metrics_percentile() {
        let mut m = XiMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xi_metrics_merge() {
        let mut a = XiMetrics::new("a");
        a.record(1.0);
        let mut b = XiMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xi_metrics_reset() {
        let mut m = XiMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xi_rate_window_empty() {
        let rw = XiRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xi_rate_window_tick_and_rate() {
        let mut rw = XiRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xi_lru_cache_basic() {
        let mut c = XiLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xi_lru_cache_contains_and_keys() {
        let mut c = XiLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xi_lru_cache_remove() {
        let mut c = XiLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xi_metrics_sum() {
        let mut m = XiMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xi_metrics_label() {
        let m = XiMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xi_lru_cache_clear() {
        let mut c = XiLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_26_push_and_len() {
        let mut rb = super::XbRingBuffer26::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_26_overwrite() {
        let mut rb = super::XbRingBuffer26::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_26_get_out_of_bounds() {
        let rb = super::XbRingBuffer26::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_26_drain_all() {
        let mut rb = super::XbRingBuffer26::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_26_peek_front_back() {
        let mut rb = super::XbRingBuffer26::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_26_clear() {
        let mut rb = super::XbRingBuffer26::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_26_capacity() {
        let rb = super::XbRingBuffer26::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_26_basic() {
        let h = super::xb_fnv1a_26(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_26(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_26_different_inputs() {
        let h1 = super::xb_fnv1a_26(b"abc");
        let h2 = super::xb_fnv1a_26(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_26_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_26(&data);
        let dec = super::xb_rle_decode_26(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_26_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_26(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_26(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_26_values() {
        assert!((super::xb_clamp_26(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_26(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_26(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_26_values() {
        assert!((super::xb_lerp_26(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_26(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_26(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_26_wrap_around_twice() {
        let mut rb = super::XbRingBuffer26::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 123 ----

    #[test]
    fn xc_123_pool_new_empty() {
        let pool: super::Xc123Pool<i32> = super::Xc123Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_123_pool_release_acquire() {
        let mut pool = super::Xc123Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_123_pool_acquire_empty() {
        let mut pool: super::Xc123Pool<i32> = super::Xc123Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_123_pool_full() {
        let mut pool = super::Xc123Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_123_pool_drain() {
        let mut pool = super::Xc123Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_123_pool_stats() {
        let mut pool = super::Xc123Pool::new(8);
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
    fn xc_123_pool_clear() {
        let mut pool = super::Xc123Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_123_pool_shrink() {
        let mut pool = super::Xc123Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_123_pool_default() {
        let pool: super::Xc123Pool<String> = super::Xc123Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_123_pool_extend() {
        let mut pool = super::Xc123Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_123_pool_retain() {
        let mut pool = super::Xc123Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_123_scheduler_round_robin() {
        let mut sched = super::Xc123Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_123_scheduler_empty() {
        let mut sched = super::Xc123Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_123_scheduler_reset() {
        let mut sched = super::Xc123Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_123_scheduler_add_remove() {
        let mut sched = super::Xc123Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_123_scheduler_targets() {
        let sched = super::Xc123Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_123_hash_empty() {
        assert_eq!(super::xc_123_hash(b""), 5381);
    }

    #[test]
    fn xc_123_hash_data() {
        let h = super::xc_123_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_123_hash(b"hello"), h);
    }

    #[test]
    fn xc_123_reverse_str() {
        assert_eq!(super::xc_123_reverse("abc"), "cba");
        assert_eq!(super::xc_123_reverse(""), "");
    }


    #[test]
    fn xe_38_pipeline_empty() {
        let p = super::Xe38Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_38_pipeline_parse_stage() {
        let p = super::Xe38Pipeline::new()
            .add_parse(super::xe_38_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_38_pipeline_transform_double() {
        let p = super::Xe38Pipeline::new()
            .add_transform(super::xe_38_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_38_pipeline_validate_reverse() {
        let p = super::Xe38Pipeline::new()
            .add_validate(super::xe_38_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_38_pipeline_emit_filter() {
        let p = super::Xe38Pipeline::new()
            .add_emit(super::xe_38_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_38_pipeline_multi_stage() {
        let p = super::Xe38Pipeline::new()
            .add_parse(super::xe_38_pipeline_identity)
            .add_transform(super::xe_38_pipeline_double)
            .add_validate(super::xe_38_pipeline_reverse)
            .add_emit(super::xe_38_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_38_pipeline_error_propagation() {
        let p = super::Xe38Pipeline::new()
            .add_parse(super::xe_38_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe38Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_38_pipeline_compose() {
        let p1 = super::Xe38Pipeline::new()
            .add_parse(super::xe_38_pipeline_identity);
        let p2 = super::Xe38Pipeline::new()
            .add_transform(super::xe_38_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_38_pipeline_error_display() {
        let e = super::Xe38PipelineError {
            stage: super::Xe38Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_38_cache_put_get() {
        let mut c = super::Xe38Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_38_cache_miss() {
        let mut c: super::Xe38Cache<&str, i32> = super::Xe38Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_38_cache_ttl_expiry() {
        let mut c = super::Xe38Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_38_cache_evict() {
        let mut c = super::Xe38Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_38_cache_capacity() {
        let mut c = super::Xe38Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_38_cache_stats() {
        let mut c = super::Xe38Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_38_cache_clear() {
        let mut c = super::Xe38Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_5 graph tests ------------------------------------------------

    #[test]
    fn xg_5_graph_empty() {
        let g = super::Xg5Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_5_graph_add_node() {
        let mut g = super::Xg5Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_5_graph_add_edge() {
        let mut g = super::Xg5Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_5_graph_neighbors() {
        let mut g = super::Xg5Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_5_graph_has_path() {
        let mut g = super::Xg5Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_5_graph_self_path() {
        let g = super::Xg5Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_5_graph_topo_sort() {
        let mut g = super::Xg5Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_5_graph_cycle_detect_false() {
        let mut g = super::Xg5Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_5_graph_cycle_detect_true() {
        let mut g = super::Xg5Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_5 heap tests -------------------------------------------------

    #[test]
    fn xg_5_heap_empty() {
        let h: super::Xg5Heap<i32> = super::Xg5Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_5_heap_push_pop() {
        let mut h = super::Xg5Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_5_heap_peek() {
        let mut h = super::Xg5Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_5_heap_drain_sorted() {
        let mut h = super::Xg5Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_5_heap_merge() {
        let mut a = super::Xg5Heap::new();
        let mut b = super::Xg5Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_5_heap_default() {
        let h: super::Xg5Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_5_graph_default() {
        let g: super::Xg5Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh122_skip_insert_contains() {
        let mut sl = super::Xh122SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh122_skip_remove() {
        let mut sl = super::Xh122SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh122_skip_len() {
        let mut sl = super::Xh122SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh122_skip_range_query() {
        let mut sl = super::Xh122SkipList::xh_new(4);
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
    fn xh122_skip_floor_ceiling() {
        let mut sl = super::Xh122SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh122_skip_rank() {
        let mut sl = super::Xh122SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh122_skip_empty() {
        let sl = super::Xh122SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh122_skip_duplicates() {
        let mut sl = super::Xh122SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh122_bitset_set_test() {
        let mut bs = super::Xh122BitSet::xh_new(256);
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
    fn xh122_bitset_clear_count() {
        let mut bs = super::Xh122BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh122_bitset_and_or_xor() {
        let mut a = super::Xh122BitSet::xh_new(128);
        let mut b = super::Xh122BitSet::xh_new(128);
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
    fn xh122_bitset_iter_ones() {
        let mut bs = super::Xh122BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh122_bitset_first_last() {
        let mut bs = super::Xh122BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh122_bitset_empty() {
        let bs = super::Xh122BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi122_deque_push_pop_back() {
        let mut dq = super::Xi122Deque::xi_new(4);
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
    fn xi122_deque_push_pop_front() {
        let mut dq = super::Xi122Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi122_deque_mixed_ops() {
        let mut dq = super::Xi122Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi122_deque_get_and_split() {
        let mut dq = super::Xi122Deque::xi_new(8);
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
    fn xi122_deque_rotate_left() {
        let mut dq = super::Xi122Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi122_deque_rotate_right() {
        let mut dq = super::Xi122Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi122_deque_grow() {
        let mut dq = super::Xi122Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi122_deque_empty() {
        let dq = super::Xi122Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi122_interval_tree_insert_query() {
        let mut tree = super::Xi122IntervalTree::xi_new();
        tree.xi_insert(super::Xi122Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi122Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi122Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi122_interval_tree_overlap() {
        let mut tree = super::Xi122IntervalTree::xi_new();
        tree.xi_insert(super::Xi122Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi122Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi122Interval::xi_new(12, 20));
        let q = super::Xi122Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi122_interval_tree_remove() {
        let mut tree = super::Xi122IntervalTree::xi_new();
        tree.xi_insert(super::Xi122Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi122Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi122_interval_tree_gaps() {
        let mut tree = super::Xi122IntervalTree::xi_new();
        tree.xi_insert(super::Xi122Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi122Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi122Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi122Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi122Interval::xi_new(8, 10));
    }

    #[test]
    fn xi122_interval_tree_merge() {
        let mut tree = super::Xi122IntervalTree::xi_new();
        tree.xi_insert(super::Xi122Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi122Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi122Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi122Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi122Interval::xi_new(10, 15));
    }

    #[test]
    fn xi122_interval_tree_all() {
        let mut tree = super::Xi122IntervalTree::xi_new();
        tree.xi_insert(super::Xi122Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi122Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi122_interval_tree_empty() {
        let tree = super::Xi122IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi122_interval_tree_contains_point() {
        let iv = super::Xi122Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 122) ---

    #[test]
    fn xj_122_uf_make_and_find() {
        let mut uf = super::Xj122UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_122_uf_union_connected() {
        let mut uf = super::Xj122UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_122_uf_component_count() {
        let mut uf = super::Xj122UnionFind::xj_new();
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
    fn xj_122_uf_component_size() {
        let mut uf = super::Xj122UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_122_uf_largest_component() {
        let mut uf = super::Xj122UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_122_uf_many_elements() {
        let mut uf = super::Xj122UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_122_uf_separate_components() {
        let mut uf = super::Xj122UnionFind::xj_new();
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
    fn xj_122_uf_path_compression() {
        let mut uf = super::Xj122UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_122_bt_insert_get() {
        let mut bt = super::Xj122BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_122_bt_contains_len() {
        let mut bt = super::Xj122BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_122_bt_replace() {
        let mut bt = super::Xj122BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_122_bt_remove() {
        let mut bt = super::Xj122BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_122_bt_keys_values() {
        let mut bt = super::Xj122BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_122_bt_range() {
        let mut bt = super::Xj122BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_122_bt_min_max() {
        let mut bt = super::Xj122BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_122_bt_many_inserts() {
        let mut bt = super::Xj122BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_122 segment tree tests ---

    #[test]
    fn xk_122_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk122SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_122_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk122SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_122_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk122SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_122_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk122SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_122_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk122SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_122_st_single_element() {
        let data = vec![42];
        let st = super::Xk122SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_122_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk122SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_122_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk122SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_122 disjoint intervals tests ---

    #[test]
    fn xk_122_di_add_and_count() {
        let mut di = super::Xk122DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_122_di_merge_overlap() {
        let mut di = super::Xk122DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_122_di_contains() {
        let mut di = super::Xk122DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_122_di_remove() {
        let mut di = super::Xk122DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_122_di_covered_length() {
        let mut di = super::Xk122DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_122_di_gaps() {
        let mut di = super::Xk122DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_122_di_merge_adjacent() {
        let mut di = super::Xk122DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_122_di_empty() {
        let di = super::Xk122DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}