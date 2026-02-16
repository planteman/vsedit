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
}
