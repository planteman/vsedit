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

}