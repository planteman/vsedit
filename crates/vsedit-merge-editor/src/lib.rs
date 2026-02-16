//! 3-way merge editor.

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
}
