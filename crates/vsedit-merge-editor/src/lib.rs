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


/// Rope data structure for efficient large text manipulation (xl_122).
#[derive(Debug, Clone)]
pub struct Xl122Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl122Rope {
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

/// Suffix array for efficient string searching (xl_122).
#[derive(Debug, Clone)]
pub struct Xl122SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl122SuffixArray {
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
pub struct Xm122MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm122MatrixSparse {
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
pub struct Xm122Tokenizer {
    text: String,
}

impl Xm122Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 122.
pub struct Xn122Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn122Fenwick {
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

// ----- AVL tree map — crate 122 -----

#[derive(Debug, Clone)]
struct Xn122AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn122AvlNode<K, V>>>,
    right: Option<Box<Xn122AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 122.
#[derive(Debug, Clone)]
pub struct Xn122AVL<K, V> {
    root: Option<Box<Xn122AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn122AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn122AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn122AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn122AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn122AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn122AvlNode<K, V>>) -> Box<Xn122AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn122AvlNode<K, V>>) -> Box<Xn122AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn122AvlNode<K, V>>) -> Box<Xn122AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn122AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn122AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn122AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn122AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn122AvlNode<K, V>>) -> &Xn122AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn122AvlNode<K, V>>) -> (Box<Xn122AvlNode<K, V>>, Option<Box<Xn122AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn122AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn122AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn122AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn122AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn122AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn122AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn122AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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


// ---------------------------------------------------------------------------
// Xo122RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo122Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo122RBNode<K, V> {
    key: K,
    value: V,
    color: Xo122Color,
    left: Option<Box<Xo122RBNode<K, V>>>,
    right: Option<Box<Xo122RBNode<K, V>>>,
}

/// A red-black tree map for crate 122.
#[derive(Debug, Clone)]
pub struct Xo122RedBlack<K, V> {
    root: Option<Box<Xo122RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo122RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo122Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo122RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo122RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo122RBNode {
                    key, value, color: Xo122Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo122RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo122Color::Red)
    }

    fn xo_balance(mut h: Box<Xo122RBNode<K, V>>) -> Box<Xo122RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo122Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo122RBNode<K, V>>) -> Box<Xo122RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo122Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo122RBNode<K, V>>) -> Box<Xo122RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo122Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo122RBNode<K, V>>) {
        h.color = Xo122Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo122Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo122Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo122Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo122RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo122RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo122RBNode<K, V>) -> (K, V, Option<Box<Xo122RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo122RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo122Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo122RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo122ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 122.
#[derive(Debug, Clone)]
pub struct Xo122ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo122ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo122#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo122#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 122).
#[derive(Debug)]
pub struct Xp122SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp122Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp122Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp122Node<K, V>>>,
    xp_right: Option<Box<Xp122Node<K, V>>>,
}

impl<K: Ord, V> Xp122Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp122SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp122SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp122Node<K, V>>>, key: &K) -> Option<Box<Xp122Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp122Node<K, V>>) -> Box<Xp122Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp122Node<K, V>>) -> Box<Xp122Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp122Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp122Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp122Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq122Treap ---------------

use std::cmp::Ordering as Xq122Ord;

struct Xq122TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq122TreapNode<K, V>>>,
    right: Option<Box<Xq122TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq122Treap<K, V> {
    root: Option<Box<Xq122TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq122TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_122_size<K, V>(node: &Option<Box<Xq122TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_122_update_size<K, V>(node: &mut Xq122TreapNode<K, V>) {
    node.size = 1 + xq_122_size(&node.left) + xq_122_size(&node.right);
}

fn xq_122_rotate_right<K, V>(mut node: Box<Xq122TreapNode<K, V>>) -> Box<Xq122TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_122_update_size(&mut node);
    left.right = Some(node);
    xq_122_update_size(&mut left);
    left
}

fn xq_122_rotate_left<K, V>(mut node: Box<Xq122TreapNode<K, V>>) -> Box<Xq122TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_122_update_size(&mut node);
    right.left = Some(node);
    xq_122_update_size(&mut right);
    right
}

fn xq_122_insert_node<K: Ord, V>(
    node: Option<Box<Xq122TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq122TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq122TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq122Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq122Ord::Less => {
                let (new_left, old) = xq_122_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_122_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_122_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq122Ord::Greater => {
                let (new_right, old) = xq_122_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_122_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_122_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_122_remove_node<K: Ord, V>(
    node: Option<Box<Xq122TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq122TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq122Ord::Less => {
                let (new_left, old) = xq_122_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_122_update_size(&mut n);
                (Some(n), old)
            }
            Xq122Ord::Greater => {
                let (new_right, old) = xq_122_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_122_update_size(&mut n);
                (Some(n), old)
            }
            Xq122Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_122_rotate_right(n);
                    let (new_right, old) = xq_122_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_122_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_122_rotate_left(n);
                    let (new_left, old) = xq_122_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_122_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_122_find_min<K, V>(node: &Option<Box<Xq122TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_122_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_122_find_max<K, V>(node: &Option<Box<Xq122TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_122_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_122_rank<K: Ord, V>(node: &Option<Box<Xq122TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq122Ord::Less => xq_122_rank(&n.left, key),
            Xq122Ord::Equal => xq_122_size(&n.left),
            Xq122Ord::Greater => 1 + xq_122_size(&n.left) + xq_122_rank(&n.right, key),
        },
    }
}

fn xq_122_kth<K, V>(node: &Option<Box<Xq122TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_122_size(&n.left);
        if k < left_size {
            xq_122_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_122_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_122_in_order<K: Clone, V>(node: &Option<Box<Xq122TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_122_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_122_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq122Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 122 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_122_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq122Ord::Equal => return Some(&n.value),
                Xq122Ord::Less => cur = &n.left,
                Xq122Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_122_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_122_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_122_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_122_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_122_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_122_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_122_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq122VEBTree ---------------

pub struct Xq122VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq122VEBTree>>,
    clusters: Vec<Option<Box<Xq122VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq122VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq122VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq122VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr122KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr122KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr122BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr122KDNode {
    xr_point: Xr122KDPoint,
    xr_left: Option<Box<Xr122KDNode>>,
    xr_right: Option<Box<Xr122KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr122KDTree {
    xr_root: Option<Box<Xr122KDNode>>,
    xr_size: usize,
}

impl Xr122KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr122KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr122KDNode>>,
        point: Xr122KDPoint,
        depth: usize,
    ) -> Box<Xr122KDNode> {
        match node {
            None => Box::new(Xr122KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr122KDPoint) -> Option<Xr122KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr122KDNode>,
        query: &Xr122KDPoint,
        depth: usize,
        best: &mut Xr122KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr122KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr122KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr122KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr122KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr122KDNode>>, pts: &mut Vec<Xr122KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr122KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr122BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr122BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs122PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs122PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs122PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs122PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs122ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs122ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs122ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs122RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs122RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs122RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs122CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs122CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs122CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}

/// Auxiliary statistics tracker for xs_122 data structures.
#[derive(Debug, Clone)]
pub struct Xs122StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs122StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs122StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
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


    #[test]
    fn xl_122_rope_new_empty() {
        let rope = super::Xl122Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_122_rope_from_str() {
        let rope = super::Xl122Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_122_rope_insert_at() {
        let mut rope = super::Xl122Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_122_rope_delete_range() {
        let mut rope = super::Xl122Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_122_rope_char_at() {
        let rope = super::Xl122Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_122_rope_split_concat() {
        let rope = super::Xl122Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_122_rope_line_count() {
        let rope = super::Xl122Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_122_rope_line_at() {
        let rope = super::Xl122Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_122_sa_build_and_search() {
        let sa = super::Xl122SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_122_sa_count() {
        let sa = super::Xl122SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_122_sa_longest_repeated() {
        let sa = super::Xl122SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_122_sa_all_positions() {
        let sa = super::Xl122SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_122_sa_len() {
        let sa = super::Xl122SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_122_sa_empty() {
        let sa = super::Xl122SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_122_rope_slice() {
        let rope = super::Xl122Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_122_sa_search_start() {
        let sa = super::Xl122SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_122_sparse_set_get() {
        let mut m = super::Xm122MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_122_sparse_row_col() {
        let mut m = super::Xm122MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_122_sparse_transpose() {
        let mut m = super::Xm122MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_122_sparse_multiply_vec() {
        let mut m = super::Xm122MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_122_sparse_nnz_density() {
        let mut m = super::Xm122MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_122_sparse_clear() {
        let mut m = super::Xm122MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_122_sparse_overwrite_zero() {
        let mut m = super::Xm122MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_122_tokenizer_basic() {
        let t = super::Xm122Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_122_tokenizer_count() {
        let t = super::Xm122Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_122_tokenizer_unique() {
        let t = super::Xm122Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_122_tokenizer_frequency() {
        let t = super::Xm122Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_122_tokenizer_delimiter() {
        let t = super::Xm122Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_122_tokenizer_whitespace() {
        let t = super::Xm122Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_122_tokenizer_empty() {
        let t = super::Xm122Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 122 ----

    #[test]
    fn xn_122_fenwick_prefix_sum() {
        let mut ft = super::Xn122Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_122_fenwick_range_sum() {
        let mut ft = super::Xn122Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_122_fenwick_point_query() {
        let mut ft = super::Xn122Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_122_fenwick_len() {
        let ft = super::Xn122Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_122_fenwick_multiple_updates() {
        let mut ft = super::Xn122Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_122_fenwick_single_element() {
        let mut ft = super::Xn122Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_122_fenwick_find_kth() {
        let mut ft = super::Xn122Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_122_fenwick_negative_delta() {
        let mut ft = super::Xn122Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 122 ----

    #[test]
    fn xn_122_avl_insert_get() {
        let mut m = super::Xn122AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_122_avl_remove() {
        let mut m = super::Xn122AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_122_avl_in_order() {
        let mut m = super::Xn122AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_122_avl_min_max() {
        let mut m = super::Xn122AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_122_avl_floor_ceiling() {
        let mut m = super::Xn122AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_122_avl_height_balanced() {
        let mut m = super::Xn122AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_122_avl_overwrite() {
        let mut m = super::Xn122AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_122_avl_empty() {
        let m: super::Xn122AVL<i32, i32> = super::Xn122AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo122RedBlack tests ---

    #[test]
    fn xo_122_rb_insert_and_get() {
        let mut tree = super::Xo122RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_122_rb_len_and_empty() {
        let mut tree = super::Xo122RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_122_rb_min_max() {
        let mut tree = super::Xo122RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_122_rb_contains() {
        let mut tree = super::Xo122RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_122_rb_remove() {
        let mut tree = super::Xo122RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_122_rb_in_order() {
        let mut tree = super::Xo122RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_122_rb_black_height() {
        let mut tree = super::Xo122RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_122_rb_overwrite() {
        let mut tree = super::Xo122RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo122ConsistentHash tests ---

    #[test]
    fn xo_122_ch_add_and_count() {
        let mut ring = super::Xo122ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_122_ch_remove_node() {
        let mut ring = super::Xo122ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_122_ch_get_node() {
        let mut ring = super::Xo122ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_122_ch_empty_ring() {
        let ring = super::Xo122ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_122_ch_distribution() {
        let mut ring = super::Xo122ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_122_ch_rebalance() {
        let mut ring = super::Xo122ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_122_ch_virtual_nodes() {
        let mut ring = super::Xo122ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_122_ch_consistent_lookup() {
        let mut ring = super::Xo122ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_122_splay_insert_get() {
        let mut t = super::Xp122SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_122_splay_remove() {
        let mut t = super::Xp122SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_122_splay_count_increases() {
        let mut t = super::Xp122SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_122_splay_depth() {
        let mut t = super::Xp122SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_122_splay_len_empty() {
        let t = super::Xp122SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_122_splay_min_max() {
        let mut t = super::Xp122SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_122_splay_overwrite() {
        let mut t = super::Xp122SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_122_splay_remove_missing() {
        let mut t = super::Xp122SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_122 treap tests ----
    #[test]
    fn xq_122_treap_empty() {
        let t = super::Xq122Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_122_treap_insert_get() {
        let mut t = super::Xq122Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_122_treap_overwrite() {
        let mut t = super::Xq122Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_122_treap_remove() {
        let mut t = super::Xq122Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_122_treap_min_max() {
        let mut t = super::Xq122Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_122_treap_rank() {
        let mut t = super::Xq122Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_122_treap_kth() {
        let mut t = super::Xq122Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_122_treap_in_order() {
        let mut t = super::Xq122Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_122 VEB tree tests ----
    #[test]
    fn xq_122_veb_empty() {
        let v = super::Xq122VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_122_veb_insert_contains() {
        let mut v = super::Xq122VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_122_veb_min_max() {
        let mut v = super::Xq122VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_122_veb_delete() {
        let mut v = super::Xq122VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_122_veb_successor() {
        let mut v = super::Xq122VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_122_veb_predecessor() {
        let mut v = super::Xq122VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_122_veb_count() {
        let mut v = super::Xq122VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_122_veb_duplicate_insert() {
        let mut v = super::Xq122VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_122_kdtree_empty() {
        let tree = super::Xr122KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_122_kdtree_insert_one() {
        let mut tree = super::Xr122KDTree::xr_new();
        tree.xr_insert(super::Xr122KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_122_kdtree_insert_multiple() {
        let mut tree = super::Xr122KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr122KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_122_kdtree_nearest_neighbor() {
        let mut tree = super::Xr122KDTree::xr_new();
        tree.xr_insert(super::Xr122KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr122KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr122KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_122_kdtree_nn_empty() {
        let tree = super::Xr122KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr122KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_122_kdtree_range_search() {
        let mut tree = super::Xr122KDTree::xr_new();
        tree.xr_insert(super::Xr122KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr122KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr122KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_122_kdtree_range_empty() {
        let mut tree = super::Xr122KDTree::xr_new();
        tree.xr_insert(super::Xr122KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_122_kdtree_all_points() {
        let mut tree = super::Xr122KDTree::xr_new();
        tree.xr_insert(super::Xr122KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr122KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_122_kdtree_depth() {
        let mut tree = super::Xr122KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr122KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_122_kdtree_bounding_box() {
        let mut tree = super::Xr122KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr122KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr122KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_122_persistent_array_new() {
        let arr = super::Xs122PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_122_persistent_array_push() {
        let mut arr = super::Xs122PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_122_persistent_array_set() {
        let mut arr = super::Xs122PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_122_persistent_array_diff() {
        let mut arr = super::Xs122PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_122_persistent_array_rollback() {
        let mut arr = super::Xs122PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_122_persistent_array_history() {
        let mut arr = super::Xs122PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_122_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs122PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_122_persistent_array_from_vec() {
        let arr = super::Xs122PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_122_concurrent_queue_new() {
        let q = super::Xs122ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_122_concurrent_queue_push_pop() {
        let mut q = super::Xs122ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_122_concurrent_queue_full() {
        let mut q = super::Xs122ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_122_concurrent_queue_drain() {
        let mut q = super::Xs122ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_122_concurrent_queue_try_pop() {
        let mut q = super::Xs122ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_122_concurrent_queue_clear() {
        let mut q = super::Xs122ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_122_range_map_new() {
        let rm = super::Xs122RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_122_range_map_insert_get() {
        let mut rm = super::Xs122RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_122_range_map_overlap() {
        let mut rm = super::Xs122RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_122_range_map_remove() {
        let mut rm = super::Xs122RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_122_range_map_gaps() {
        let mut rm = super::Xs122RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_122_range_map_coverage() {
        let mut rm = super::Xs122RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_122_range_map_contains() {
        let mut rm = super::Xs122RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_122_range_map_clear() {
        let mut rm = super::Xs122RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_122_circular_buffer_new() {
        let buf = super::Xs122CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_122_circular_buffer_push_pop() {
        let mut buf = super::Xs122CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_122_circular_buffer_overwrite() {
        let mut buf = super::Xs122CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_122_circular_buffer_peek() {
        let mut buf = super::Xs122CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_122_circular_buffer_is_full() {
        let mut buf = super::Xs122CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_122_circular_buffer_iter() {
        let mut buf = super::Xs122CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_122_circular_buffer_clear() {
        let mut buf = super::Xs122CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_122_circular_buffer_to_vec() {
        let mut buf = super::Xs122CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_122_stats_tracker_new() {
        let tracker = super::Xs122StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_122_stats_tracker_mean() {
        let mut tracker = super::Xs122StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_122_stats_tracker_min_max() {
        let mut tracker = super::Xs122StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_122_stats_tracker_median() {
        let mut tracker = super::Xs122StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_122_stats_tracker_variance() {
        let mut tracker = super::Xs122StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_122_stats_tracker_range() {
        let mut tracker = super::Xs122StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_122_stats_tracker_clear() {
        let mut tracker = super::Xs122StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_122_stats_tracker_sum() {
        let mut tracker = super::Xs122StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }

}