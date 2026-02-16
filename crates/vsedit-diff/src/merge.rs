//! Three-way merge conflict detection and parsing.

/// A single merge conflict region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    /// The base (common ancestor) text, if available (diff3 style).
    pub base: String,
    /// The current (ours) side.
    pub current: String,
    /// The incoming (theirs) side.
    pub incoming: String,
    /// Line range in the source file (start, end) — 0-based.
    pub range: (u32, u32),
}

/// Action a user can take to resolve a merge conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeAction {
    AcceptCurrent,
    AcceptIncoming,
    AcceptBoth,
    CompareChanges,
}

/// Parse merge conflict markers from text.
///
/// Detects `<<<<<<<`, `=======`, `>>>>>>>` markers and optional diff3 `|||||||` base markers.
pub fn parse_merge_conflicts(text: &str) -> Vec<MergeConflict> {
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

            // Collect current side, watching for optional ||||||| base marker
            while i < lines.len()
                && !lines[i].starts_with("=======")
                && !lines[i].starts_with("|||||||")
            {
                current_lines.push(lines[i]);
                i += 1;
            }

            // Optional diff3 base section
            if i < lines.len() && lines[i].starts_with("|||||||") {
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

            // Collect incoming side
            while i < lines.len() && !lines[i].starts_with(">>>>>>>") {
                incoming_lines.push(lines[i]);
                i += 1;
            }

            let end = i as u32;

            conflicts.push(MergeConflict {
                base: base_lines.join("\n"),
                current: current_lines.join("\n"),
                incoming: incoming_lines.join("\n"),
                range: (start, end),
            });
        }
        i += 1;
    }

    conflicts
}

/// Resolve a merge conflict by applying the given action.
pub fn resolve_conflict(conflict: &MergeConflict, action: MergeAction) -> String {
    match action {
        MergeAction::AcceptCurrent => conflict.current.clone(),
        MergeAction::AcceptIncoming => conflict.incoming.clone(),
        MergeAction::AcceptBoth => {
            if conflict.current.is_empty() {
                conflict.incoming.clone()
            } else if conflict.incoming.is_empty() {
                conflict.current.clone()
            } else {
                format!("{}\n{}", conflict.current, conflict.incoming)
            }
        }
        MergeAction::CompareChanges => {
            // Returns the conflict text as-is for comparison
            format!(
                "<<<<<<< current\n{}\n=======\n{}\n>>>>>>> incoming",
                conflict.current, conflict.incoming
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_conflict() {
        let text = "before\n<<<<<<< HEAD\ncurrent\n=======\nincoming\n>>>>>>> branch\nafter";
        let conflicts = parse_merge_conflicts(text);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].current, "current");
        assert_eq!(conflicts[0].incoming, "incoming");
        assert_eq!(conflicts[0].base, "");
        assert_eq!(conflicts[0].range, (1, 5));
    }

    #[test]
    fn parse_diff3_conflict() {
        let text =
            "<<<<<<< HEAD\nours\n||||||| base\noriginal\n=======\ntheirs\n>>>>>>> branch";
        let conflicts = parse_merge_conflicts(text);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].current, "ours");
        assert_eq!(conflicts[0].base, "original");
        assert_eq!(conflicts[0].incoming, "theirs");
    }

    #[test]
    fn parse_multiple_conflicts() {
        let text = "\
<<<<<<< HEAD
a
=======
b
>>>>>>> branch
text between
<<<<<<< HEAD
c
=======
d
>>>>>>> branch";
        let conflicts = parse_merge_conflicts(text);
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].current, "a");
        assert_eq!(conflicts[1].incoming, "d");
    }

    #[test]
    fn parse_no_conflicts() {
        let text = "just normal text\nno markers here\n";
        let conflicts = parse_merge_conflicts(text);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn parse_multiline_conflict() {
        let text = "<<<<<<< HEAD\nline1\nline2\n=======\nline3\nline4\nline5\n>>>>>>> branch";
        let conflicts = parse_merge_conflicts(text);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].current, "line1\nline2");
        assert_eq!(conflicts[0].incoming, "line3\nline4\nline5");
    }

    #[test]
    fn resolve_accept_current() {
        let c = MergeConflict {
            base: String::new(),
            current: "ours".into(),
            incoming: "theirs".into(),
            range: (0, 4),
        };
        assert_eq!(resolve_conflict(&c, MergeAction::AcceptCurrent), "ours");
    }

    #[test]
    fn resolve_accept_incoming() {
        let c = MergeConflict {
            base: String::new(),
            current: "ours".into(),
            incoming: "theirs".into(),
            range: (0, 4),
        };
        assert_eq!(resolve_conflict(&c, MergeAction::AcceptIncoming), "theirs");
    }

    #[test]
    fn resolve_accept_both() {
        let c = MergeConflict {
            base: String::new(),
            current: "ours".into(),
            incoming: "theirs".into(),
            range: (0, 4),
        };
        assert_eq!(
            resolve_conflict(&c, MergeAction::AcceptBoth),
            "ours\ntheirs"
        );
    }

    #[test]
    fn resolve_compare_changes() {
        let c = MergeConflict {
            base: String::new(),
            current: "ours".into(),
            incoming: "theirs".into(),
            range: (0, 4),
        };
        let result = resolve_conflict(&c, MergeAction::CompareChanges);
        assert!(result.contains("<<<<<<< current"));
        assert!(result.contains(">>>>>>> incoming"));
    }
}
